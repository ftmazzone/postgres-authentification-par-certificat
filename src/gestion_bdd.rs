use configuration_bdd::ConfigurationBdd;
use futures::channel::mpsc;
use futures::{join, stream, FutureExt, StreamExt, TryStreamExt};
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time;
use tokio::time::{sleep, timeout};
use tokio_postgres::{AsyncMessage, Config};

use crate::configuration_bdd;

pub async fn demarrer_lecture_notifications(
    operationnel: &Arc<AtomicBool>,
    configuration_bdd: ConfigurationBdd,
    connector: postgres_native_tls::MakeTlsConnector,
) -> Result<(), tokio_postgres::Error> {
    let (client, mut connection) = Config::new()
        .host(&configuration_bdd.adresse)
        .port(configuration_bdd.port)
        .user(&configuration_bdd.utilisateur)
        .dbname(&configuration_bdd.base_de_donnees)
        .application_name(&configuration_bdd.application)
        .connect(connector)
        .await?;

    //Utilisant l'exemple de la page : https://github.com/sfackler/rust-postgres/blob/fc10985f9fdf0903893109bc951fb5891539bf97/tokio-postgres/tests/test/main.rs#L612
    let (tx, mut rx) = mpsc::unbounded();
    let stream = stream::poll_fn(
        move |cx| -> std::task::Poll<
            std::option::Option<
                std::result::Result<tokio_postgres::AsyncMessage, tokio_postgres::Error>,
            >,
        > {
            let message = connection.poll_message(cx);
            match message {
                std::task::Poll::Ready(m) => match m {
                    Some(i) => match i {
                        Ok(n) => std::task::Poll::Ready(Some(Result::Ok(n))),
                        Err(e) => {
                            println!("erreur {:?}", e);
                            return std::task::Poll::Ready(None);
                        }
                    },
                    None => std::task::Poll::Ready(None),
                },
                std::task::Poll::Pending => message,
            }
        },
    )
    .map_err(|e| panic!("{}", e));

    let connection = stream.forward(tx).map(|r| r.unwrap());
    let mut tache = tokio::spawn(connection);

    client
        .batch_execute(
            "LISTEN mes_notifications;
         NOTIFY mes_notifications, 'ma_premiere_notification';
         NOTIFY mes_notifications, 'ma_seconde_notification';",
        )
        .await
        .unwrap();

    let mut mes_notifications = Vec::<tokio_postgres::Notification>::new();

    let lire_version_resultat = lire_version(&client);
    let lire_notifications_resultat =
        lire_notifications(&operationnel, &mut rx, &mut mes_notifications, &mut tache);
    let (t1, t2) = join!(lire_version_resultat, lire_notifications_resultat);

    if t1.is_ok() && t2.is_ok() {
        println!(
            "Etape 1 : notification : {} {:?}",
            &mut mes_notifications.len(),
            &mut rx
        );
        mes_notifications = Vec::<tokio_postgres::Notification>::new();
        let lire_version_resultat = lire_version(&client);
        let lire_notifications_resultat =
            lire_notifications(&operationnel, &mut rx, &mut mes_notifications, &mut tache);
        let (t1, t2) = join!(lire_version_resultat, lire_notifications_resultat);

        if t1.is_ok() && t2.is_ok() {
            println!(
                "Etape 2 : notification : {} {:?}",
                &mut mes_notifications.len(),
                &mut rx
            );
        }
    } else {
        println!("Erreur lors de l'exécution : {:?}", t2);
    }
    rx.close();

    Ok(())
}

async fn lire_version(client: &tokio_postgres::Client) -> Result<(), tokio_postgres::Error> {
    let lignes = client
        .query("SELECT $1::TEXT || version()", &[&"Connecté à "])
        .await?;

    let version: &str = lignes[0].get(0);
    println!("{}", version);
    sleep(time::Duration::from_secs(5)).await;

    Ok(())
}

async fn lire_notifications(
    operationnel: &Arc<AtomicBool>,
    rx: &mut mpsc::UnboundedReceiver<AsyncMessage>,
    mes_notifications: &mut Vec<tokio_postgres::Notification>,
    mut tache: &mut tokio::task::JoinHandle<()>,
) -> Result<(), Box<dyn Error>> {
    println!("Presser '^C' pour quitter la boucle\n");

    let mut connexion_bdd_active = true;

    while operationnel.load(Ordering::SeqCst) && connexion_bdd_active {
        let resultat_attente = timeout(time::Duration::from_secs(5), rx.next()).await;

        let connexion_bdd = (&mut tache).now_or_never();
        if let Some(Ok(m)) = connexion_bdd {
            println!("Attente fermeture de la tache : {:?}", m);
            connexion_bdd_active = false;
        }

        if let Ok(Some(m)) = resultat_attente {
            println!("{:?}", m);
            match m {
                AsyncMessage::Notification(n) => {
                    mes_notifications.push(n);
                }
                _ => {}
            }
        }
    }
    if connexion_bdd_active {
        return Ok(());
    } else {
        return Err("Erreur d'exécution de la méthode 'lire_notifications'".into());
    }
}
