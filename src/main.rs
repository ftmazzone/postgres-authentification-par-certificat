use native_tls::{Certificate, Identity, TlsConnector};
use postgres_native_tls::MakeTlsConnector;
use tokio_postgres::{Config, Error};
use serde::{Deserialize, Serialize};
use std::fs;

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
    lire_configuration();
    let configuration_bdd = lire_configuration();

    //fichier crt
    let cert = fs::read(configuration_bdd.certificat_serveur).unwrap();
    let cert = Certificate::from_pem(&cert).unwrap();

    //fichier pfx et mot de passe du fichier pfx
    let certificat_client = fs::read(configuration_bdd.certificat_client).unwrap();
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

    let (client, connection) = Config::new()
        .host(&configuration_bdd.adresse)
        .port(configuration_bdd.port)
        .user(&configuration_bdd.utilisateur)
        .dbname(&configuration_bdd.base_de_donnees)
        .application_name(&configuration_bdd.application)
        .connect(connector)
        .await?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    let lignes = client
        .query("SELECT $1::TEXT || version()", &[&"Connecté à "])
        .await?;

    let version: &str = lignes[0].get(0);
    println!("{}", version);
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