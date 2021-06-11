use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Deserialize, Serialize,Clone)]
pub struct ConfigurationBdd {
    pub adresse: String,
    pub port: u16,
    pub utilisateur: String,
    #[serde(rename = "motDePasse")]
    pub mot_de_passe: Option<String>,
    #[serde(rename = "baseDeDonnees")]
    pub base_de_donnees: String,
    pub application: String,
    #[serde(rename = "certificatServeur")]
    pub certificat_serveur: String,
    #[serde(rename = "certificatClient")]
    pub certificat_client: Option<String>,
    #[serde(rename = "motDePasseCertificatClient")]
    pub mot_de_passe_certificat_client: Option<String>,
}

impl Default for ConfigurationBdd {
    fn default() -> ConfigurationBdd {
        let nom_programme = std::env::current_exe()
            .expect("L'adresse du programme ne peut être obtenue.")
            .file_name()
            .expect("Le nom du programme ne peut être obtenue.")
            .to_string_lossy()
            .into_owned();

        ConfigurationBdd {
            adresse: "localhost".to_string(),
            port: 5432,
            utilisateur: "postgres".to_string(),
            mot_de_passe: Some("******".to_string()),
            base_de_donnees: "postgres".to_string(),
            application: nom_programme.to_string(),
            certificat_serveur: "./certificats/serveur.crt".to_string(),
            certificat_client: Some("./certificats/client.pfx".to_string()),
            mot_de_passe_certificat_client: Some("******".to_string()),
        }
    }
}

pub fn lire_configuration() -> ConfigurationBdd {
    let mut configuration_bdd: ConfigurationBdd = Default::default();
    let configuration;
    match fs::read_to_string("configuration.json") {
        Ok(c) => configuration = c,
        Err(e) => match e.kind() {
            std::io::ErrorKind::NotFound => {
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
                }
            }
            _ => panic!(
                "Erreur lors de la lecture du fichier de configuration : {:#?}",
                e
            ),
        },
    }

    match serde_json::from_str::<ConfigurationBdd>(&configuration) {
        Ok(m) => configuration_bdd = m,
        Err(e) => panic!(
            "Erreur lors de la désérialisation du fichier de configuration : {:#?}",
            e
        ),
    }
    configuration_bdd
}
