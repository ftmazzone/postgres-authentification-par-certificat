use native_tls::{Certificate, Identity, TlsConnector};
use postgres_native_tls::MakeTlsConnector;
use simple_signal::{self, Signal};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio_postgres::{Error};
use std::{fs};
use configuration_bdd::{lire_configuration};
use gestion_bdd::{demarrer_lecture_notifications};

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



