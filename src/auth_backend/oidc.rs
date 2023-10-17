use std::collections::HashMap;
use std::string::ToString;

use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use constant_time_eq::constant_time_eq;
use openidconnect::{AccessTokenHash, AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce, OAuth2TokenResponse, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse};
use openidconnect::core::{CoreAuthenticationFlow, CoreClient, CoreJsonWebKeySet, CoreProviderMetadata};
use openidconnect::reqwest::async_http_client;
use serde::{Deserialize, Serialize};

use crate::auth::ROUTE_CALLBACK_USER_AUTH;
use crate::auth_backend::{AuthBackend, AuthCache, LoginType, UserInfo};
use crate::config::config::Config;
use crate::config::oidc;

const SESSION_KEY_OIDC_STATE: &str = "oidc_state";
const SESSION_KEY_OIDC_NONCE: &str = "oidc_nonce";
const SESSION_KEY_OIDC_PKCE: &str = "oidc_pkce";

#[derive(Deserialize)]
struct OidcParams {
    state: String,
    session_state: String,
    code: String,
}

pub struct Oidc {
    pub(crate) config: oidc::Oidc,
}

#[derive(Serialize, Deserialize)]
struct Metadata {
    provider: Vec<u8>,
    jwks: Vec<u8>,
}

impl Oidc {
    pub fn new(config: &Config) -> Self {
        Self {
            config: config.oidc.clone(),
        }
    }

    fn get_client(&self, host: &str, cache: &AuthCache) -> Result<CoreClient> {
        let metadata: Metadata = serde_json::from_slice(cache)?;
        let jwks: CoreJsonWebKeySet  = serde_json::from_slice(&metadata.jwks)?;
        let mut provider_metadata: CoreProviderMetadata = serde_json::from_slice(&metadata.provider)?;
        provider_metadata = provider_metadata.set_jwks(jwks);
        Ok(
            CoreClient::from_provider_metadata(
                provider_metadata,
                ClientId::new(self.config.client_id.clone()),
                Some(ClientSecret::new(self.config.client_secret.clone())),
            ).set_redirect_uri(
                RedirectUrl::new(
                    format!("{}{}", host, ROUTE_CALLBACK_USER_AUTH).to_string()
                )?
            )
        )
    }
}

#[async_trait]
impl AuthBackend for Oidc {
    fn validate_config(&self) -> Result<()> {
        self.config.validate()
    }

    async fn init(&self) -> Result<AuthCache> {
        let provider_metadata = CoreProviderMetadata::discover_async(
            IssuerUrl::from_url(self.config.issuer.clone().unwrap()),
            async_http_client,
        ).await?;

        let provider = serde_json::to_vec(&provider_metadata)?;
        // jwks is skipped in serialization, so we have to serialize it separately
        let jwks = serde_json::to_vec(provider_metadata.jwks())?;

        Ok(
            serde_json::to_vec(&Metadata {
                provider,
                jwks,
            })?
        )
    }

    fn get_login_type(&self, host: &str, cache: &AuthCache) -> Result<LoginType> {
        let client = self.get_client(host, cache)?;

        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        let mut req = client.authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        ).set_pkce_challenge(pkce_challenge);

        for scope in &self.config.scopes {
            req = req.add_scope(Scope::new(scope.clone()));
        }

        let (auth_url, state, nonce) = req.url();

        let to_session = HashMap::from([
            (SESSION_KEY_OIDC_STATE.to_string(), state.secret().to_owned()),
            (SESSION_KEY_OIDC_NONCE.to_string(), nonce.secret().to_owned()),
            (SESSION_KEY_OIDC_PKCE.to_string(), pkce_verifier.secret().to_owned()),
        ]);

        Ok(
            LoginType::Redirect {
                url: auth_url,
                to_session,
            }
        )
    }

    fn get_session_keys(&self, _: &AuthCache) -> Result<Vec<String>> {
        Ok(
            vec![
                SESSION_KEY_OIDC_STATE.to_string(),
                SESSION_KEY_OIDC_NONCE.to_string(),
                SESSION_KEY_OIDC_PKCE.to_string(),
            ]
        )
    }

    async fn callback(&self, mut from_session: HashMap<String, String>, cache: &AuthCache, params: serde_json::Value, host: &str) -> Result<UserInfo> {
        let oidc_params: OidcParams = serde_json::from_value(params)?;

        let state = from_session.remove(SESSION_KEY_OIDC_STATE).ok_or(anyhow!("failed to get csrf token from session"))?;
        if !constant_time_eq(oidc_params.state.as_bytes(), state.as_bytes()) {
            bail!("invalid csrf token (state)");
        }

        let client = self.get_client(host, cache)?;
        let pkce_verifier = from_session.remove(SESSION_KEY_OIDC_PKCE).ok_or(anyhow!("failed to get pkce from session"))?;
        let token_response = client
            .exchange_code(AuthorizationCode::new(oidc_params.code))
            .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier))
            .request_async(async_http_client)
            .await?;

        let id_token = token_response
            .id_token()
            .ok_or(anyhow!("server did not return an ID token"))?;

        let nonce = from_session.remove(SESSION_KEY_OIDC_NONCE).ok_or(anyhow!("failed to get nonce from session"))?;
        let claims = id_token.claims(&client.id_token_verifier(), &Nonce::new(nonce))?;

        if let Some(expected_access_token_hash) = claims.access_token_hash() {
            let actual_access_token_hash = AccessTokenHash::from_token(
                token_response.access_token(),
                &id_token.signing_alg()?,
            )?;
            if !constant_time_eq(actual_access_token_hash.as_bytes(), expected_access_token_hash.as_bytes()) {
                bail!("invalid access token");
            }
        } else {
            bail!("access token hash is missing");
        }

        let id = claims.subject().as_str().to_owned();
        let name = match claims.preferred_username() {
            None => id.clone(),
            Some(n) => n.as_str().to_owned(),
        };

        Ok(
            UserInfo {
                id,
                name,
            }
        )
    }
}