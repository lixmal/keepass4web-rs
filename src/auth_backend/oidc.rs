use std::str::FromStr;
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
    IdToken,
    IdTokenFields,
    IssuerUrl,
    LogoutRequest,
    Nonce,
    OAuth2TokenResponse,
    PkceCodeChallenge,
    PkceCodeVerifier,
    PostLogoutRedirectUrl,
    ProviderMetadataWithLogout,
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
    CoreIdToken,
    CoreJsonWebKey,
    CoreJsonWebKeyType,
    CoreJsonWebKeyUse,
    CoreJweContentEncryptionAlgorithm,
    CoreJwsSigningAlgorithm,
    CoreRevocableToken,
    CoreRevocationErrorResponse,
    CoreTokenIntrospectionResponse,
    CoreTokenType,
};
use openidconnect::reqwest::async_http_client;
use serde::{Deserialize, Serialize};

use crate::auth_backend::{AuthBackend, AuthCache, LoginType, LogoutType, ROUTE_CALLBACK_USER_AUTH, UserInfo};
use crate::config::config::Config;
use crate::config::oidc;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
struct KeePassClaims {
    database_location: Option<String>,
    keyfile_location: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct State {
    state: String,
    nonce: String,
    pkce: String,
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
        let provider_metadata = Self::get_metadata(cache)?;

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

    fn get_metadata(cache: &AuthCache) -> Result<&ProviderMetadataWithLogout> {
        match cache.downcast_ref::<ProviderMetadataWithLogout>() {
            Some(v) => Ok(v),
            None => bail!("failed to retrieve provider metadata from cache"),
        }
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
                ProviderMetadataWithLogout::discover_async(
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

        let state = State {
            state: state.secret().to_owned(),
            nonce: nonce.secret().to_owned(),
            pkce: pkce_verifier.secret().to_owned(),
        };

        Ok(
            LoginType::Redirect {
                url: auth_url,
                state: serde_json::to_string(&state)?,
            }
        )
    }

    fn get_logout_type(&self, user_info: &UserInfo, _host: &str, _cache: &AuthCache) -> Result<LogoutType> {
        let provider_metadata = Self::get_metadata(_cache)?;
        let logout_endpoint = provider_metadata.additional_metadata().end_session_endpoint.
            clone().ok_or(anyhow!("no session endpoint defined"))?;

        let mut logout_request = LogoutRequest::from(logout_endpoint)
            .set_post_logout_redirect_uri(
                PostLogoutRedirectUrl::new(_host.to_string())?
            )
            .set_client_id(ClientId::new(self.config.client_id.clone()));

        if let Some(id_token) = &user_info.additional_data {
            let token: CoreIdToken = IdToken::from_str(id_token)?;
            logout_request = logout_request.set_id_token_hint(&token);
        }

        Ok(
            LogoutType::Redirect {
                url: logout_request.http_get_url(),
            }
        )
    }

    async fn callback(&self, from_session: String, cache: &AuthCache, params: serde_json::Value, host: &str) -> Result<UserInfo> {
        let oidc_params: OidcParams = serde_json::from_value(params)?;

        if let Some(err) = oidc_params.error {
            if let Some(err_desc) = oidc_params.error_description {
                bail!("error from auth server: {}: {}", err, err_desc)
            } else {
                bail!("error from auth server: {}", err)
            }
        }

        let state: State = serde_json::from_str(&from_session)?;
        if !constant_time_eq(oidc_params.state.as_bytes(), state.state.as_bytes()) {
            bail!("invalid csrf token (state)");
        }

        let client = self.get_client(host, cache)?;
        let token_response = client
            .exchange_code(AuthorizationCode::new(oidc_params.code))
            .set_pkce_verifier(PkceCodeVerifier::new(state.pkce))
            .request_async(async_http_client)
            .await?;

        let id_token = token_response
            .id_token()
            .ok_or(anyhow!("server did not return an ID token"))?;

        let claims = id_token.claims(&client.id_token_verifier(), &Nonce::new(state.nonce))?;

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

        let mut additional_data: Option<String> = None;
        if self.config.save_id_token {
            additional_data = Some(id_token.to_string());
        }

        Ok(
            UserInfo {
                id,
                name,
                db_location: claims.additional_claims().database_location.clone(),
                keyfile_location: claims.additional_claims().keyfile_location.clone(),
                additional_data,
            }
        )
    }
}