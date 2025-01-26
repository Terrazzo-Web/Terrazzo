pub mod terrazzo {
    pub mod gateway {
        include!(concat!(env!("OUT_DIR"), "/terrazzo.gateway.rs"));
    }
}
