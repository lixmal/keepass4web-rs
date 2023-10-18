use std::collections::HashMap;
use std::string::ToString;

use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use constant_time_eq::constant_time_eq;
use openidconnect::{
    AccessTokenHash,
    AdditionalClaims,
    AuthorizationCode,
    Client,
    ClientId,
    ClientSecret,
    CsrfToken,
    EmptyExtraTokenFields,
    IdTokenFields,
    IssuerUrl,
    Nonce,
    OAuth2TokenResponse,
    PkceCodeChallenge,
    PkceCodeVerifier,
    RedirectUrl,
    Scope,
    StandardErrorResponse,
    StandardTokenResponse,
    TokenResponse,
};
use openidconnect::core::{
    CoreAuthDisplay,
    CoreAuthenticationFlow,
    CoreAuthPrompt,
    CoreErrorResponseType,
    CoreGenderClaim,
    CoreJsonWebKey,
    CoreJsonWebKeyType,
    CoreJsonWebKeyUse,
    CoreJweContentEncryptionAlgorithm,
    CoreJwsSigningAlgorithm,
    CoreProviderMetadata,
    CoreRevocableToken,
    CoreRevocationErrorResponse,
    CoreTokenIntrospectionResponse,
    CoreTokenType,
};
use openidconnect::reqwest::async_http_client;
use serde::{Deserialize, Serialize};

use crate::auth::ROUTE_CALLBACK_USER_AUTH;
use crate::auth_backend::{AuthBackend, AuthCache, LoginType, UserInfo};
use crate::config::config::Config;
use crate::config::oidc;

const SESSION_KEY_OIDC_STATE: &str = "oidc_state";
const SESSION_KEY_OIDC_NONCE: &str = "oidc_nonce";
const SESSION_KEY_OIDC_PKCE: &str = "oidc_pkce";

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
struct KeePassClaims {
    database_location: Option<String>,
    keyfile_location: Option<String>,
}

impl AdditionalClaims for KeePassClaims {}

type OidcClient = Client<
    KeePassClaims,
    CoreAuthDisplay,
    CoreGenderClaim,
    CoreJweContentEncryptionAlgorithm,
    CoreJwsSigningAlgorithm,
    CoreJsonWebKeyType,
    CoreJsonWebKeyUse,
    CoreJsonWebKey,
    CoreAuthPrompt,
    StandardErrorResponse<CoreErrorResponseType>,
    StandardTokenResponse<
        IdTokenFields<
            KeePassClaims,
            EmptyExtraTokenFields,
            CoreGenderClaim,
            CoreJweContentEncryptionAlgorithm,
            CoreJwsSigningAlgorithm,
            CoreJsonWebKeyType
        >,
        CoreTokenType
    >,
    CoreTokenType,
    CoreTokenIntrospectionResponse,
    CoreRevocableToken,
    CoreRevocationErrorResponse,
>;

#[derive(Deserialize)]
struct OidcParams {
    state: String,
    session_state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
    code: String,
}

pub struct Oidc {
    pub(crate) config: oidc::Oidc,
}

impl Oidc {
    pub fn new(config: &Config) -> Self {
        Self {
            config: config.oidc.clone(),
        }
    }

    fn get_client(&self, host: &str, cache: &AuthCache) -> Result<OidcClient> {
        let provider_metadata = match cache.downcast_ref::<CoreProviderMetadata>() {
            Some(v) => v,
            None => bail!("failed to retrieve provider metadata from cache"),
        };

        let client: OidcClient = Client::new(
            ClientId::new(self.config.client_id.clone()),
            Some(ClientSecret::new(self.config.client_secret.clone())),
            provider_metadata.issuer().clone(),
            provider_metadata.authorization_endpoint().clone(),
            provider_metadata.token_endpoint().cloned(),
            provider_metadata.userinfo_endpoint().cloned(),
            provider_metadata.jwks().to_owned(),
        ).set_redirect_uri(
            RedirectUrl::new(
                format!("{}{}", host, ROUTE_CALLBACK_USER_AUTH).to_string()
            )?
        );
        Ok(client)
    }
}

#[async_trait]
impl AuthBackend for Oidc {
    fn validate_config(&self) -> Result<()> {
        self.config.validate()
    }

    async fn init(&self) -> Result<AuthCache> {
        Ok(
            Box::new(
                CoreProviderMetadata::discover_async(
                    IssuerUrl::from_url(self.config.issuer.clone().unwrap()),
                    async_http_client,
                ).await?
            )
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

        if let Some(err) = oidc_params.error {
            if let Some(err_desc) = oidc_params.error_description {
                bail!("error from auth server: {}: {}", err, err_desc)
            } else {
                bail!("error from auth server: {}", err)
            }
        }

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
                db_location: claims.additional_claims().database_location.clone(),
                keyfile_location: claims.additional_claims().keyfile_location.clone(),
            }
        )
    }
}