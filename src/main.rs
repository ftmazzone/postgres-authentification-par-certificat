use configuration_bdd::lire_configuration;
use futures::join;
use native_tls::{Identity, TlsConnector};
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

    let certificat_autorite_base_de_donnees =
        fs::read(configuration_bdd.certificat_autorite_bdd.to_string()).expect(&format!(
            "Erreur lecture fichier {}",
            configuration_bdd.certificat_autorite_bdd
        ));
    let certificat_autorite_base_de_donnees =
        native_tls::Certificate::from_pem(&certificat_autorite_base_de_donnees).expect(&format!(
            "Erreur lecture fichier {}",
            configuration_bdd.certificat_autorite_bdd
        ));

    let mut connector_builder = TlsConnector::builder();
    connector_builder.add_root_certificate(certificat_autorite_base_de_donnees);

    let certificat_client_base_de_donnees =
        fs::read(configuration_bdd.certificat_client.as_ref().unwrap()).expect(&format!(
            "Erreur lecture fichier certificat client de la base de données{:?}",
            configuration_bdd.certificat_client
        ));
    let certificat_client_base_de_donnees_clef =
        fs::read(configuration_bdd.certificat_client_clef.as_ref().unwrap()).expect(&format!(
            "Erreur lecture fichier clef du certificat client de la base de données {:?}",
            configuration_bdd.certificat_client_clef
        ));
    let certificat_client_base_de_donnees = Identity::from_pkcs8(
        &certificat_client_base_de_donnees,
        &certificat_client_base_de_donnees_clef,
    )
    .unwrap();
    connector_builder.identity(certificat_client_base_de_donnees);

    let connecteur_tls = connector_builder.build().unwrap();
    let connecteur = MakeTlsConnector::new(connecteur_tls.clone());
    let connecteur_flume = MakeTlsConnector::new(connecteur_tls.clone());
    let operationnel_flume = operationnel.clone();

    let (_resultat1, _resultat2) = join!(
        lecture_notifications::demarrer(&operationnel, configuration_bdd.clone(), connecteur,),
        lecture_notifications_flume::demarrer(
            &operationnel_flume,
            configuration_bdd.clone(),
            connecteur_flume
        )
    );

    Ok(())
}
