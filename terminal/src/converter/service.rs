#![cfg(feature = "server")]

use std::sync::Arc;

use futures::StreamExt as _;
use futures::channel::mpsc;
use nameth::nameth;
use server_fn::BoxedStream;
use server_fn::ServerFnError;
use terrazzo::declare_trait_aliias;
use tonic::Status;

use super::api::Conversion;
use super::api::Conversions;
use crate::converter::api::Language;

mod asn1;
mod base64;
mod dns;
mod json;
mod jwt;
mod pkcs7;
mod timestamps;
mod tls_info;
mod unescaped;
mod x509;

#[nameth]
#[allow(dead_code)]
pub async fn get_conversions(input: Arc<str>) -> Result<Conversions, Status> {
    let mut stream = stream_conversions(input);
    let mut conversions = vec![];
    while let Some(next) = stream.next().await {
        conversions.push(next.map_err(|error| Status::internal(error.to_string()))?);
    }
    Ok(Conversions {
        conversions: conversions.into(),
    })
}

pub fn stream_conversions(input: Arc<str>) -> BoxedStream<Conversion, ServerFnError> {
    let (tx, rx) = mpsc::unbounded();
    tokio::spawn(async move {
        produce_conversions(input, tx).await;
    });
    BoxedStream::from(rx.map(Ok))
}

async fn produce_conversions(input: Arc<str>, tx: mpsc::UnboundedSender<Conversion>) {
    if self::x509::add_x509_pem(&input, &mut add_conversion(tx.clone())) {
        return;
    }
    if self::jwt::add_jwt(&input, &mut add_conversion(tx.clone())) {
        return;
    }
    if self::base64::add_base64(&input, &mut add_conversion(tx.clone())) {
        return;
    }
    {
        let mut add = add_conversion(tx.clone());
        if !self::json::add_json(&input, &mut add) {
            self::json::add_yaml(&input, &mut add);
        }
    }
    self::unescaped::add_unescape(&input, &mut add_conversion(tx.clone()));
    if self::tls_info::add_tls_info(&input, &mut add_conversion(tx.clone())).await {
        return;
    }
    let dns_input = input.clone();
    let timestamps_input = input.clone();
    let dns_tx = tx.clone();
    let timestamps_tx = tx.clone();
    tokio::join!(
        async move {
            let mut add = add_conversion(dns_tx);
            let _ = self::dns::add_dns(&dns_input, &mut add).await;
        },
        async move {
            let mut add = add_conversion(timestamps_tx);
            self::timestamps::add_timestamps(&timestamps_input, &mut add);
        }
    );
}

fn add_conversion(
    tx: mpsc::UnboundedSender<Conversion>,
) -> impl FnMut(Language, String) + Send + 'static {
    move |language, content| {
        let _ = tx.unbounded_send(Conversion::new(language, content));
    }
}

declare_trait_aliias!(AddConversionFn, FnMut(Language, String));

#[cfg(test)]
mod tests {

    pub trait GetConversionForTest {
        async fn get_conversion(&self, language: &str) -> String;
        async fn get_languages(&self) -> Vec<String>;
    }

    impl GetConversionForTest for &str {
        async fn get_conversion(&self, language: &str) -> String {
            let conversions = super::get_conversions(self.to_string().into())
                .await
                .unwrap();
            let matches = conversions
                .conversions
                .iter()
                .filter(|conversion| conversion.language.name.as_ref() == language)
                .collect::<Vec<_>>();
            match *matches.as_slice() {
                [] => "Not found".to_string(),
                [conversion] => conversion.content.clone(),
                _ => "Duplicates".to_string(),
            }
        }

        async fn get_languages(&self) -> Vec<String> {
            let conversions = super::get_conversions(self.to_string().into())
                .await
                .unwrap();
            let mut languages = conversions
                .conversions
                .iter()
                .map(|conversion| conversion.language.name.to_string())
                .collect::<Vec<_>>();
            languages.sort();
            return languages;
        }
    }
}
