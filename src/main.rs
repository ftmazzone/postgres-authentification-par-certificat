use native_tls::{Certificate, Identity, TlsConnector};
use postgres_native_tls::MakeTlsConnector;
use serde::{Deserialize, Serialize};
use simple_signal::{self, Signal};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{fs, time};
use tokio::time::{sleep, timeout};
use tokio_postgres::{AsyncMessage, Config, Error};

use futures::channel::mpsc;
use futures::{join, stream, FutureExt, StreamExt, TryStreamExt};

#[derive(Deserialize, Serialize)]
struct ConfigurationBdd {
    adresse: String,
    port: u16,
    utilisateur: String,
    #[serde(rename = "motDePasse")]
    mot_de_passe: String,
    #[serde(rename = "baseDeDonnees")]
    base_de_donnees: String,
    application: String,
    #[serde(rename = "certificatServeur")]
    certificat_serveur: String,
    #[serde(rename = "certificatClient")]
    certificat_client: String,
    #[serde(rename = "motDePasseCertificatClient")]
    mot_de_passe_certificat_client: String,
}

impl Default for ConfigurationBdd {
    fn default() -> ConfigurationBdd {
        ConfigurationBdd {
            adresse: "localhost".to_string(),
            port: 5432,
            utilisateur: "postgres".to_string(),
            mot_de_passe: "******".to_string(),
            base_de_donnees: "postgres".to_string(),
            application: "mon_application".to_string(),
            certificat_serveur: "./certificats/serveur.crt".to_string(),
            certificat_client: "./certificats/client.pfx".to_string(),
            mot_de_passe_certificat_client: "******".to_string(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let operationnel = Arc::new(AtomicBool::new(true));
    let r = operationnel.clone();
    simple_signal::set_handler(&[Signal::Int, Signal::Term], move |signal_recu| {
        println!("Signal reçu : '{:?}'", signal_recu);
        r.store(false, Ordering::SeqCst);
    });

    lire_configuration();
    let configuration_bdd = lire_configuration();

    //fichier crt
    let cert = fs::read(configuration_bdd.certificat_serveur.to_owned()).unwrap();
    let cert = Certificate::from_pem(&cert).unwrap();

    //fichier pfx et mot de passe du fichier pfx
    let certificat_client = fs::read(configuration_bdd.certificat_client.to_owned()).unwrap();
    let certificat_client = Identity::from_pkcs12(
        &certificat_client,
        &configuration_bdd.mot_de_passe_certificat_client,
    )
    .unwrap();

    let connector = TlsConnector::builder()
        .add_root_certificate(cert)
        .identity(certificat_client)
        .build()
        .unwrap();

    let connector = MakeTlsConnector::new(connector);
    demarrer_lecture_notifications(&operationnel, configuration_bdd, connector).await?;

    Ok(())
}

async fn demarrer_lecture_notifications(
    operationnel: &Arc<AtomicBool>,
    configuration_bdd: ConfigurationBdd,
    connector: postgres_native_tls::MakeTlsConnector,
) -> Result<(), Error> {
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
) -> Result<(), Error> {
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

fn lire_configuration() -> ConfigurationBdd {
    let mut configuration_bdd: ConfigurationBdd = Default::default();
    let configuration;
    match fs::read_to_string("configuration.json") {
        Ok(c) => configuration = c,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                let fichier_configuration_texte =
                    serde_json::to_string_pretty(&configuration_bdd).unwrap();
                match fs::write("configuration.json", fichier_configuration_texte) {
                    Ok(m) => {
                        println!(
                            "Fichier configuration exemple créé : 'configuration.json' : {:#?}",
                            m
                        );

                        configuration = serde_json::to_string(&configuration_bdd).unwrap();
                    }

                    Err(e) => panic!(
                        "Erreur lors de la lecture du fichier de configuration : {:#?}",
                        e
                    ),
                };
            } else {
                panic!(
                    "Erreur lors de la lecture du fichier de configuration : {:#?}",
                    e
                );
            }
        }
    }

    match serde_json::from_str::<ConfigurationBdd>(&configuration) {
        Ok(m) => configuration_bdd = m,
        Err(e) => {
            panic!(
                "Erreur lors de la désérialisation du fichier de configuration : {:#?}",
                e
            )
        }
    }
    configuration_bdd
}
