use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationBdd {
    pub adresse: String,
    pub port: u16,
    pub utilisateur: String,
    pub mot_de_passe: Option<String>,
    pub base_de_donnees: String,
    pub application: String,
    pub certificat_autorite_bdd: String,
    pub certificat_client: Option<String>,
    pub certificat_client_clef: Option<String>,
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
            certificat_autorite_bdd: "./certificats/serveur.crt".to_string(),
            certificat_client: Some("./certificats/client.crt".to_string()),
            certificat_client_clef: Some("./certificats/client.key".to_string()),
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
