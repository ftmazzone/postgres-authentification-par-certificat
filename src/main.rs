use configuration_bdd::lire_configuration;
use gestion_bdd::demarrer_lecture_notifications;
use native_tls::{Certificate, Identity, TlsConnector};
use postgres_native_tls::MakeTlsConnector;
use simple_signal::{self, Signal};
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio_postgres::Error;

mod configuration_bdd;
mod gestion_bdd;

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

    let connector = MakeTlsConnector::new(connecteur_tls);
    demarrer_lecture_notifications(&operationnel, configuration_bdd, connector).await?;

    Ok(())
}
