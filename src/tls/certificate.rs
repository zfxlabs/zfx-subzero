use derive_more::{Display, Error, From};
use pem::Pem;
use pem::PemError;
use rcgen::RcgenError;
use rcgen::{Certificate, CertificateParams, KeyIdMethod, KeyPair, PKCS_ED25519};
use std::{fs, path::Path};
use x509_parser::certificate::X509Certificate;
use x509_parser::error::X509Error;
use x509_parser::prelude::FromDer;

/// Checks and returns the identity derived from `cert_file` and `priv_key_file` if found,
/// otherwise generates it and writes the certificate and key to the supplied paths
pub fn get_node_cert(cert_file: &Path, priv_key_file: &Path) -> Result<(Vec<u8>, Vec<u8>)> {
    if cert_file.exists() && priv_key_file.exists() {
        let cert: Vec<u8> = pem::parse(fs::read(cert_file)?)?.contents;
        let key = pem::parse(fs::read(priv_key_file)?)?.contents;
        // Quick check if certificate just loaded is a valid one
        let (_rest, _cert) = X509Certificate::from_der(&cert)?;
        Ok((cert, key))
    } else {
        // No certificate at the given path, generate one
        let (cert, priv_key) = generate_node_cert()?;
        let pem_cert = der_to_pem(&cert, "CERTIFICATE");
        let pem_key = der_to_pem(&priv_key, "PRIVATE KEY");
        fs::write(cert_file, &pem_cert)?;
        fs::write(priv_key_file, &pem_key)?;
        Ok((cert, priv_key))
    }
}

/// Generate a valid, self signed X.509 certificate using the hash of the public key as name
///
pub fn generate_node_cert() -> Result<(Vec<u8>, Vec<u8>)> {
    let alg = &PKCS_ED25519;
    let key_pair = KeyPair::generate(alg)?;
    let san = "zfx-node".to_owned();
    let mut params = CertificateParams::new(vec![san]);
    params.alg = alg;
    params.key_pair = Some(key_pair);
    params.key_identifier_method = KeyIdMethod::Sha256;

    let cert = Certificate::from_params(params)?;
    let private_key = cert.serialize_private_key_der();
    let cert = cert.serialize_der()?;
    Ok((cert, private_key))
}

/// Convenience wrapper around `pem::encode(&Pem)`
#[inline]
fn der_to_pem(contents: &[u8], tag: &str) -> String {
    let pem = Pem { tag: String::from(tag), contents: contents.to_owned() };
    pem::encode(&pem)
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug, Display, From)]
pub enum Error {
    IoError(std::io::Error),
    TryFromStringError,
    CertificateGenError(RcgenError),
    CertificateReadError(PemError),
    CertificateParseError(x509_parser::nom::Err<X509Error>),
}

#[cfg(test)]
mod test {
    use super::*;
    use std::env::temp_dir;
    use std::path::PathBuf;

    fn rand_fname() -> String {
        use rand::{distributions::Alphanumeric, thread_rng, Rng};

        let mut rng = thread_rng();
        std::iter::repeat(()).map(|()| rng.sample(Alphanumeric) as char).take(8).collect()
    }

    #[actix_rt::test]
    async fn get_twice() {
        let fname = rand_fname();
        let crt = generate_file_in_tmp_dir(&fname, String::from("crt"));
        let key = generate_file_in_tmp_dir(&fname, String::from("key"));

        let stuff1 = get_node_cert(&crt, &key).unwrap();
        let stuff2 = get_node_cert(&crt, &key).unwrap();
        assert_eq!(stuff1, stuff2);
    }

    #[actix_rt::test]
    async fn rcgen_test() {
        let cert = rcgen::generate_simple_self_signed(vec!["foo".to_string()]).unwrap();
        let private_key = cert.serialize_private_key_pem();
        let cert = cert.serialize_pem().unwrap();
        let fname = rand_fname();
        let cert_file = generate_file_in_tmp_dir(&fname, String::from("crt"));
        let priv_key_file = generate_file_in_tmp_dir(&fname, String::from("key"));

        fs::write(&cert_file, cert).unwrap();
        fs::write(&priv_key_file, private_key).unwrap();

        // Read certificate & secret key back
        matches!(get_node_cert(&cert_file, &priv_key_file), Ok((_cert, _key)));
    }

    fn generate_file_in_tmp_dir(name: &String, extension: String) -> PathBuf {
        temp_dir().join(format!("{}.{}", name, extension))
    }
}
