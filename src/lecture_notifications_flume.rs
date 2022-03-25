use configuration_bdd::ConfigurationBdd;
use flume;
use futures::{join, stream, StreamExt};
use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time;
use tokio::time::{sleep, timeout};
use tokio_postgres::{AsyncMessage, Config};

use crate::configuration_bdd;

pub async fn demarrer(
    operationnel: &Arc<AtomicBool>,
    configuration_bdd: ConfigurationBdd,
    connecteur_tls: postgres_native_tls::MakeTlsConnector,
) -> Result<(), tokio_postgres::Error> {
    let mut configuration_connexion = Config::new();

    configuration_connexion
        .host(&configuration_bdd.adresse)
        .port(configuration_bdd.port)
        .user(&configuration_bdd.utilisateur)
        .dbname(&configuration_bdd.base_de_donnees)
        .application_name(&configuration_bdd.application);

    if let Some(mot_de_passe) = &configuration_bdd.mot_de_passe {
        configuration_connexion.password(mot_de_passe);
    }

    while operationnel.load(Ordering::SeqCst) {
        log::info!("Presser '^C' pour quitter la boucle\n");

        let (client, mut connection) = configuration_connexion
            .connect(connecteur_tls.clone())
            .await?;

        //Utilisant l'exemple de la page : https://github.com/sfackler/rust-postgres/blob/fc10985f9fdf0903893109bc951fb5891539bf97/tokio-postgres/tests/test/main.rs#L612
        let (tx, mut rx) = flume::unbounded();

        let erreur_connexion = Arc::new(AtomicBool::new(false));
        let erreur_connexion_thread = erreur_connexion.clone();

        let stream = stream::poll_fn(move |cx| {
            let message = connection.poll_message(cx);
            match message {
                std::task::Poll::Ready(Some(Ok(_))) => message,
                std::task::Poll::Ready(Some(Err(e))) => {
                    //Transformer les erreurs de connexions pour ne pas générer une erreur fatale
                    log::error!("erreur stream::poll_fn");
                    if !erreur_connexion.load(Ordering::SeqCst) {
                        std::task::Poll::Ready(Some(Err(e)))
                    } else {
                        std::task::Poll::Ready(None)
                    }
                }
                std::task::Poll::Ready(None) => message,
                std::task::Poll::Pending => message,
            }
        });

        let connexion = stream
            .map(move |r| match r {
                Ok(message) => Ok(Ok(message)),
                Err(erreur) => {
                    if !erreur_connexion_thread.load(Ordering::SeqCst) {
                        erreur_connexion_thread.store(true, Ordering::SeqCst);
                    }
                    Ok(Err(erreur))
                }
            })
            .forward(tx.into_sink());

        tokio::spawn(async move {
            let resultat = connexion.await;
            match resultat {
                Ok(_) => log::info!("Connexion fermée"),
                Err(_) => log::error!("Erreur lors de la fermeture de la connexion"),
            }
        });

        let mut mes_notifications = Vec::<tokio_postgres::Notification>::new();

        let lire_notifications_resultat =
            lire_notifications(&operationnel, &client, &mut rx, &mut mes_notifications);

        let lire_version_resultat = lire_version(&client);

        let (t1, t2) = join!(lire_version_resultat, lire_notifications_resultat);

        if !t1.is_ok() || !t2.is_ok() {
            sleep(time::Duration::from_secs(5)).await;
            log::error!("Taches terminée avec une ou plusieurs erreurs");
        } else {
            log::info!("Taches terminée");
        }
    }
    Ok(())
}

async fn lire_version(client: &tokio_postgres::Client) -> Result<(), tokio_postgres::Error> {
    let lignes = client
        .query(
            "SELECT $1::TEXT || version(),current_database()",
            &[&"Connecté à "],
        )
        .await?;

    let version: &str = lignes[0].get(0);
    let nom_base_de_donnees: &str = lignes[0].get(1);
    log::info!("{}. Base de données : '{}'", version, nom_base_de_donnees);
    sleep(time::Duration::from_secs(5)).await;

    Ok(())
}

async fn lire_notifications(
    operationnel: &Arc<AtomicBool>,
    client: &tokio_postgres::Client,
    rx: &mut flume::Receiver<Result<AsyncMessage, tokio_postgres::Error>>,
    mes_notifications: &mut Vec<tokio_postgres::Notification>,
) -> Result<(), ErreurLectureNotifications> {
    let mut erreur_ecoute_notification = false;
    let result = client
        .batch_execute(
            "LISTEN mes_notifications_flume;
     NOTIFY mes_notifications_flume, 'ma_premiere_notification_flume';
     NOTIFY mes_notifications_flume, 'ma_seconde_notification_flume';",
        )
        .await;

    if !result.is_ok() {
        erreur_ecoute_notification = true;
    }

    while operationnel.load(Ordering::SeqCst) && !rx.is_disconnected() {
        let resultat_attente = timeout(time::Duration::from_secs(5), rx.recv_async()).await;

        match resultat_attente {
            Ok(Ok(Ok(AsyncMessage::Notification(n)))) => {
                log::info!("Notification {:?}", n);
                mes_notifications.push(n);
            }
            Ok(Ok(Err(err))) => {
                log::error!("Erreur 1 {:?}", err);
                erreur_ecoute_notification = true;
            }
            Ok(Err(err)) => {
                log::error!("Erreur 2 {:?}", err);
                erreur_ecoute_notification = true;
            }
            Err(err) => {
                log::debug!("Erreur 3 {:?}", err);
                erreur_ecoute_notification = true;
            }
            Ok(Ok(Ok(_))) => {}
        }
    }
    if erreur_ecoute_notification {
        Err(ErreurLectureNotifications {
            details: "erreur".to_string(),
        })
    } else {
        Ok(())
    }
}

#[derive(Debug)]
pub struct ErreurLectureNotifications {
    details: String,
}

impl fmt::Display for ErreurLectureNotifications {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl Error for ErreurLectureNotifications {
    fn description(&self) -> &str {
        &self.details
    }
}
