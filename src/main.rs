use configuration_bdd::lire_configuration;
use futures::join;
use native_tls::{Certificate, Identity, TlsConnector};
use postgres_native_tls::MakeTlsConnector;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio_postgres::Error;

mod configuration_bdd;
mod lecture_notifications;
mod lecture_notifications_flume;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    let operationnel = Arc::new(AtomicBool::new(true));

    let operationnel_arret = operationnel.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();
        log::info!("Arrêt demandé");
        operationnel_arret.store(false, Ordering::SeqCst);
    });

    lire_configuration();
    let configuration_bdd = lire_configuration();

    let mut connector_builder = TlsConnector::builder();

    //fichier crt du serveur de base de données
    let cert = fs::read(configuration_bdd.certificat_serveur.to_owned()).unwrap();
    let cert = Certificate::from_pem(&cert).unwrap();
    connector_builder.add_root_certificate(cert);

    //fichier pfx et mot de passe du fichier pfx
    if let Some(ref certificat_client) = configuration_bdd.certificat_client {
        if let Some(ref mot_de_passe_certificat_client) =
            configuration_bdd.mot_de_passe_certificat_client
        {
            let certificat_client = fs::read(certificat_client.to_owned()).unwrap();
            let certificat_client =
                Identity::from_pkcs12(&certificat_client, &mot_de_passe_certificat_client).unwrap();
            connector_builder.identity(certificat_client);
        }
    }

    let connecteur_tls = connector_builder.build().unwrap();
    let connecteur = MakeTlsConnector::new(connecteur_tls.clone());
    let connecteur_flume = MakeTlsConnector::new(connecteur_tls.clone());
    let operationnel_flume = operationnel.clone();

    let (_resultat1, _resultat2) = join!(
        lecture_notifications::demarrer(
            &operationnel,
            configuration_bdd.clone(),
            connecteur,
        ),
        lecture_notifications_flume::demarrer(
            &operationnel_flume,
            configuration_bdd.clone(),
            connecteur_flume
        )
    );

    Ok(())
}
