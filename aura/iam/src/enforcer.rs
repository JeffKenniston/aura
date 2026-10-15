use crate::Svid;

pub trait SvidVerifier {
    fn verify(&self, svid: &Svid) -> Result<(), String>;
}

pub struct DefaultEnforcer;

impl SvidVerifier for DefaultEnforcer {
    fn verify(&self, svid: &Svid) -> Result<(), String> {
        if svid.spiffe_id().map(|id| id.to_string().is_empty()).unwrap_or(true) {
            return Err("Invalid SVID: Empty SPIFFE ID".into());
        }
        Ok(())
    }
}
