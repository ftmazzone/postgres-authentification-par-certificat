use configuration_bdd::ConfigurationBdd;
use futures::channel::mpsc;
use futures::{join, stream, FutureExt, StreamExt, TryStreamExt};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time;
use tokio::time::{sleep, timeout};
use tokio_postgres::{AsyncMessage, Config, Error};

use crate::configuration_bdd;

pub async fn demarrer_lecture_notifications(
    operationnel: &Arc<AtomicBool>,
    configuration_bdd: ConfigurationBdd,
    connector: postgres_native_tls::MakeTlsConnector,
) -> Result<(), Error> {
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

    let (client, mut connection) = configuration_connexion.connect(connector).await?;

    //Utilisant l'exemple de la page : https://github.com/sfackler/rust-postgres/blob/fc10985f9fdf0903893109bc951fb5891539bf97/tokio-postgres/tests/test/main.rs#L612
    let (tx, mut rx) = mpsc::unbounded();
    let stream =
        stream::poll_fn(move |cx| connection.poll_message(cx)).map_err(|e| panic!("{}", e));
    let connection = stream.forward(tx).map(|r| r.unwrap());
    tokio::spawn(connection);

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
        lire_notifications(&operationnel, &mut rx, &mut mes_notifications);
    let (t1, t2) = join!(lire_version_resultat, lire_notifications_resultat);

    if t1.is_ok() && t2.is_ok() {
        println!(
            "Etape 1 : notification : {} {:?}",
            &mut mes_notifications.len(),
            &mut rx
        );
    }

    mes_notifications = Vec::<tokio_postgres::Notification>::new();
    let lire_version_resultat = lire_version(&client);
    let lire_notifications_resultat =
        lire_notifications(&operationnel, &mut rx, &mut mes_notifications);
    let (t1, t2) = join!(lire_version_resultat, lire_notifications_resultat);

    if t1.is_ok() && t2.is_ok() {
        println!(
            "Etape 2 : notification : {} {:?}",
            &mut mes_notifications.len(),
            &mut rx
        );
    }
    rx.close();

    Ok(())
}

async fn lire_version(client: &tokio_postgres::Client) -> Result<(), Error> {
    let lignes = client
        .query(
            "SELECT $1::TEXT || version(),current_database()",
            &[&"Connecté à "],
        )
        .await?;

    let version: &str = lignes[0].get(0);
    let nom_base_de_donnees: &str = lignes[0].get(1);
    println!("{}. Base de données : '{}'", version, nom_base_de_donnees);
    sleep(time::Duration::from_secs(5)).await;

    Ok(())
}

async fn lire_notifications(
    operationnel: &Arc<AtomicBool>,
    rx: &mut mpsc::UnboundedReceiver<AsyncMessage>,
    mes_notifications: &mut Vec<tokio_postgres::Notification>,
) -> Result<(), Error> {
    println!("Presser '^C' pour quitter la boucle\n");
    while operationnel.load(Ordering::SeqCst) {
        if let Ok(Some(m)) = timeout(time::Duration::from_secs(5), rx.next()).await {
            println!("{:?}", m);
            match m {
                AsyncMessage::Notification(n) => {
                    mes_notifications.push(n);
                }
                _ => {}
            }
        }
    }
    Ok(())
}
