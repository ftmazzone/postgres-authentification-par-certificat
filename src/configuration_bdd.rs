use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Deserialize, Serialize)]
pub struct ConfigurationBdd {
    pub adresse: String,
    pub port: u16,
    pub utilisateur: String,
    #[serde(rename = "motDePasse")]
    pub mot_de_passe: String,
    #[serde(rename = "baseDeDonnees")]
    pub base_de_donnees: String,
    pub application: String,
    #[serde(rename = "certificatServeur")]
    pub certificat_serveur: String,
    #[serde(rename = "certificatClient")]
    pub certificat_client: Option<String>,
    #[serde(rename = "motDePasseCertificatClient")]
    pub mot_de_passe_certificat_client: String,
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
            certificat_client: Some("./certificats/client.pfx".to_string()),
            mot_de_passe_certificat_client: "******".to_string(),
        }
    }
}

pub fn lire_configuration() -> ConfigurationBdd {
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
