uniffi::setup_scaffolding!();

#[uniffi::export]
pub fn ping() -> String {
    "pong".into()
}
