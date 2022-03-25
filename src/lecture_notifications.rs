use configuration_bdd::ConfigurationBdd;
use futures::{stream, StreamExt};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{self, Duration};
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

    let mut client = None;

    while operationnel.load(Ordering::SeqCst) {
        log::info!("Presser '^C' pour quitter la boucle\n");

        let mut connexion = None;
        log::debug!("Connexion à la base de données");
        match configuration_connexion
            .connect(connecteur_tls.clone())
            .await
        {
            Ok((c1, c2)) => {
                log::debug!("Connecté à la base de données");

                client = Some(c1);
                connexion = Some(c2);
            }
            Err(err) => log::error!("{err}"),
        };

        let operationnel = operationnel.clone();

        let ecoute = tokio::spawn(async move {
            let mut connexion = connexion.unwrap();

            let mut stream = stream::poll_fn(move |cx| connexion.poll_message(cx));
            loop {
                match timeout(time::Duration::from_secs(30), stream.next()).await {
                    Ok(Some(Ok(AsyncMessage::Notification(message)))) => {
                        log::info!("Message reçu {:?}", message);
                    }
                    Ok(Some(Ok(message))) => {
                        log::error!("Erreur réception de message {:?}", message);
                        break;
                    }
                    Ok(Some(Err(erreur))) => {
                        log::error!("Erreur réception de message {:?}", erreur);
                        break;
                    }
                    Ok(None) => {
                        log::debug!("Pas de flux");
                        break;
                    }
                    Err(_err) => {
                        if operationnel.load(Ordering::SeqCst) {
                            break;
                        }
                    }
                }
            }
        });

        lire_version(&client.as_ref().unwrap()).await?;

        match &client
            .as_ref()
            .unwrap()
            .batch_execute(
                "LISTEN mes_notifications;
            NOTIFY mes_notifications, 'ma_premiere_notification';
            NOTIFY mes_notifications, 'ma_seconde_notification';",
            )
            .await
        {
            Ok(_) => {
                log::debug!("Abonnement aux évènements mes_notifications");
            }
            Err(err) => {
                log::error!("Erreur d'abonnement aux évènements mes_notifications {err}");
            }
        };
        let _resultat = ecoute.await;
        sleep(Duration::from_secs(30)).await
    }
    drop(client);
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
