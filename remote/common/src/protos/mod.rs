pub mod terrazzo {
    pub mod remote {
        pub mod health {
            include!(concat!(env!("OUT_DIR"), "/terrazzo.remote.health.rs"));
        }
    }
}
