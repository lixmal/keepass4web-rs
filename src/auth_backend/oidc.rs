use std::str::FromStr;
use std::string::ToString;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use log::warn;
use tokio::sync::RwLock;
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
    CoreJsonWebKeySet,
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
use reqwest::Client as HttpClient;
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
    http_client: HttpClient,
}

// how long provider metadata (incl. the JWKS) is served from cache
// before it is re-fetched on the next use
const METADATA_TTL: Duration = Duration::from_secs(10 * 60);

// lazily populated, periodically refreshed provider metadata.
// fetching at every use would hammer the provider, caching forever breaks
// logins when the provider rotates its signing keys or was down at startup.
#[derive(Default)]
pub struct MetadataCache {
    lock: RwLock<Option<CachedMetadata>>,
}

struct CachedMetadata {
    metadata: ProviderMetadataWithLogout,
    fetched: Instant,
}

impl CachedMetadata {
    fn is_fresh(&self) -> bool {
        self.fetched.elapsed() < METADATA_TTL
    }
}

impl Oidc {
    pub fn new(config: &Config) -> Self {
        Self {
            config: config.oidc.clone(),
            http_client: HttpClient::new(),
        }
    }

    fn get_client(&self, host: &str, provider_metadata: ProviderMetadataWithLogout) -> Result<OidcClient> {
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

    async fn fetch_metadata_from_url(&self, url: &str) -> Result<ProviderMetadataWithLogout> {
        let body = self.http_client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        let body = self.rewrite_server_endpoints(body, url)?;
        let metadata: ProviderMetadataWithLogout = serde_json::from_str(&body)?;

        // discover_async checks this and fetching from another url must not
        // lose it: the fetch address is an internal detail, the issuer is what
        // the tokens are trusted against, and the document names the jwks the
        // app would verify them with
        let issuer = self.config.issuer.clone()
            .ok_or(anyhow!("issuer must be set to fetch metadata from discovery_url"))?;
        if metadata.issuer().as_str().trim_end_matches('/') != issuer.as_str().trim_end_matches('/') {
            bail!(
                "provider metadata declares issuer '{}', expected '{}'",
                metadata.issuer().as_str(), issuer.as_str(),
            );
        }

        // openidconnect marks the jwks #[serde(skip)], so deserializing the
        // document by hand leaves it empty and every token would fail to
        // verify. discover_async fetches it separately; so must this path.
        let jwks_body = self.http_client
            .get(metadata.jwks_uri().url().as_str())
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        let jwks: CoreJsonWebKeySet = serde_json::from_str(&jwks_body)?;

        Ok(metadata.set_jwks(jwks))
    }

    // Keycloak returns every endpoint with its external hostname even when the
    // document is fetched from the internal one, so the app container cannot
    // reach them. The endpoints it talks to itself are pointed back at the host
    // the document came from; the ones the browser is sent to (authorization,
    // end_session) keep the external host so redirects still work. The issuer
    // is left alone: it is what the tokens are trusted against.
    fn rewrite_server_endpoints(&self, body: String, discovery_url: &str) -> Result<String> {
        let issuer = match &self.config.issuer {
            Some(u) => u,
            None => return Ok(body),
        };

        let discovery_origin = url_origin(discovery_url);
        let issuer_origin = url_origin(issuer.as_str());

        if discovery_origin == issuer_origin || discovery_origin.is_empty() || issuer_origin.is_empty() {
            return Ok(body);
        }

        let mut json: serde_json::Value = serde_json::from_str(&body)?;
        for field in [
            "token_endpoint",
            "jwks_uri",
            "userinfo_endpoint",
            "introspection_endpoint",
            "revocation_endpoint",
            "device_authorization_endpoint",
        ] {
            let Some(val) = json.get_mut(field) else { continue };
            let Some(url_str) = val.as_str() else { continue };
            *val = serde_json::Value::String(url_str.replacen(&issuer_origin, &discovery_origin, 1));
        }
        Ok(serde_json::to_string(&json)?)
    }

    async fn get_metadata(&self, cache: &AuthCache, force_refresh: bool) -> Result<ProviderMetadataWithLogout> {
        let cache = match cache.downcast_ref::<MetadataCache>() {
            Some(v) => v,
            None => bail!("failed to retrieve provider metadata cache"),
        };

        if !force_refresh {
            if let Some(cached) = cache.lock.read().await.as_ref() {
                if cached.is_fresh() {
                    return Ok(cached.metadata.clone());
                }
            }
        }

        let mut guard = cache.lock.write().await;
        // another task may have refreshed while we waited for the write lock
        if !force_refresh {
            if let Some(cached) = guard.as_ref() {
                if cached.is_fresh() {
                    return Ok(cached.metadata.clone());
                }
            }
        }

        let result = match &self.config.discovery_url {
            // discovery_url bypasses the openidconnect issuer-must-match check so the
            // app can fetch metadata from an internal URL (e.g. http://keycloak:8180)
            // while the issuer in tokens is an external URL (e.g. http://localhost:8180).
            Some(url) => self.fetch_metadata_from_url(url.as_str()).await.map_err(Into::into),
            None => ProviderMetadataWithLogout::discover_async(
                // issuer presence is enforced by validate_config at startup
                IssuerUrl::from_url(self.config.issuer.clone().unwrap()),
                async_http_client,
            ).await.map_err(Into::into),
        };

        match result {
            Ok(metadata) => {
                *guard = Some(CachedMetadata {
                    metadata: metadata.clone(),
                    fetched: Instant::now(),
                });
                Ok(metadata)
            }
            Err(err) => {
                // keep serving the stale copy if the provider is temporarily unreachable
                if let Some(cached) = guard.as_ref() {
                    warn!("OIDC provider metadata refresh failed, serving cached copy: {}", err);
                    return Ok(cached.metadata.clone());
                }
                Err(err)
            }
        }
    }
}

#[async_trait]
impl AuthBackend for Oidc {
    fn validate_config(&self) -> Result<()> {
        self.config.validate()
    }

    async fn init(&self) -> Result<AuthCache> {
        let cache: AuthCache = Box::new(MetadataCache::default());

        // best-effort warm-up: an unreachable provider must not prevent
        // startup, metadata is (re)fetched lazily when a login needs it
        if let Err(err) = self.get_metadata(&cache, false).await {
            warn!("OIDC provider discovery failed at startup (will retry on demand): {}", err);
        }

        Ok(cache)
    }

    async fn get_login_type(&self, host: &str, cache: &AuthCache) -> Result<LoginType> {
        let metadata = self.get_metadata(cache, false).await?;
        let client = self.get_client(host, metadata)?;

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

    async fn get_logout_type(&self, user_info: &UserInfo, host: &str, cache: &AuthCache) -> Result<LogoutType> {
        let provider_metadata = self.get_metadata(cache, false).await?;
        let logout_endpoint = provider_metadata.additional_metadata().end_session_endpoint.
            clone().ok_or(anyhow!("no session endpoint defined"))?;

        let mut logout_request = LogoutRequest::from(logout_endpoint)
            .set_post_logout_redirect_uri(
                PostLogoutRedirectUrl::new(host.to_string())?
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

        let metadata = self.get_metadata(cache, false).await?;
        let client = self.get_client(host, metadata)?;
        let token_response = client
            .exchange_code(AuthorizationCode::new(oidc_params.code))
            .set_pkce_verifier(PkceCodeVerifier::new(state.pkce))
            .request_async(async_http_client)
            .await?;

        let id_token = token_response
            .id_token()
            .ok_or(anyhow!("server did not return an ID token"))?;

        let nonce = Nonce::new(state.nonce);
        // declared here so the retry's claims may borrow from it below
        let refreshed_client;
        let claims = match id_token.claims(&client.id_token_verifier(), &nonce) {
            Ok(claims) => claims,
            Err(err) => {
                // the provider may have rotated its signing keys since the
                // metadata was cached: refresh once and retry
                warn!("ID token verification failed, refreshing provider metadata: {}", err);
                let metadata = self.get_metadata(cache, true).await?;
                refreshed_client = self.get_client(host, metadata)?;
                id_token.claims(&refreshed_client.id_token_verifier(), &nonce)?
            }
        };

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

fn url_origin(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .map(|u| {
            let mut s = format!("{}://{}", u.scheme(), u.host_str().unwrap_or(""));
            if let Some(port) = u.port() {
                s.push(':');
                s.push_str(&port.to_string());
            }
            s
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use url::Url;

    use super::*;

    fn backend(issuer: &str) -> Oidc {
        let mut config = Config::default();
        config.oidc.issuer = Some(Url::from_str(issuer).unwrap());
        config.oidc.client_id = "test-client".to_string();
        config.oidc.client_secret = "test-secret".to_string();
        Oidc::new(&config)
    }

    #[tokio::test]
    async fn discovery_is_lazy_and_recovers() {
        let mut server = mockito::Server::new_async().await;
        let issuer = format!("{}/", server.url());
        let oidc = backend(&issuer);

        // provider is 'down' (no mocks registered): init must still succeed,
        // login attempts surface the error per request
        let cache = oidc.init().await.unwrap();
        assert!(oidc.get_login_type("http://localhost:8080", &cache).await.is_err());

        // provider comes up: the next login attempt must succeed
        // without any re-initialization
        let discovery = json!({
            "issuer": issuer,
            "authorization_endpoint": format!("{}auth", issuer),
            "token_endpoint": format!("{}token", issuer),
            "jwks_uri": format!("{}jwks", issuer),
            "response_types_supported": ["code"],
            "subject_types_supported": ["public"],
            "id_token_signing_alg_values_supported": ["RS256"],
        });
        server.mock("GET", "/.well-known/openid-configuration")
            .with_header("content-type", "application/json")
            .with_body(discovery.to_string())
            .create_async().await;
        server.mock("GET", "/jwks")
            .with_header("content-type", "application/json")
            .with_body(r#"{"keys":[]}"#)
            .create_async().await;

        let login_type = oidc.get_login_type("http://localhost:8080", &cache).await.unwrap();
        match login_type {
            LoginType::Redirect { url, .. } => assert!(url.as_str().starts_with(&format!("{}auth", issuer))),
            _ => panic!("expected redirect login type"),
        }
    }

    #[tokio::test]
    async fn metadata_from_discovery_url_must_name_the_configured_issuer() {
        let mut server = mockito::Server::new_async().await;
        let issuer = "https://issuer.example.org/";

        let mut config = Config::default();
        config.oidc.issuer = Some(Url::from_str(issuer).unwrap());
        config.oidc.discovery_url = Some(Url::from_str(&format!("{}/metadata", server.url())).unwrap());
        config.oidc.client_id = "test-client".to_string();
        config.oidc.client_secret = "test-secret".to_string();
        let oidc = Oidc::new(&config);

        let metadata = |declared: &str| json!({
            "issuer": declared,
            "authorization_endpoint": format!("{}auth", declared),
            "token_endpoint": format!("{}token", declared),
            "jwks_uri": format!("{}jwks", declared),
            "response_types_supported": ["code"],
            "subject_types_supported": ["public"],
            "id_token_signing_alg_values_supported": ["RS256"],
        });

        // a document naming somebody else's issuer brings its own jwks with it
        let elsewhere = server.mock("GET", "/metadata")
            .with_header("content-type", "application/json")
            .with_body(metadata("https://attacker.example.org/").to_string())
            .create_async().await;

        let cache: AuthCache = Box::new(MetadataCache::default());
        assert!(oidc.get_metadata(&cache, true).await.is_err());
        elsewhere.assert_async().await;

        let matching = server.mock("GET", "/metadata")
            .with_header("content-type", "application/json")
            .with_body(metadata(issuer).to_string())
            .create_async().await;
        // jwks_uri is rewritten to the host the document came from, so the
        // keys are fetched from the mock rather than from the issuer
        let jwks = server.mock("GET", "/jwks")
            .with_header("content-type", "application/json")
            .with_body(json!({ "keys": [] }).to_string())
            .create_async().await;

        assert!(oidc.get_metadata(&cache, true).await.is_ok());
        matching.assert_async().await;
        jwks.assert_async().await;
    }

    #[tokio::test]
    async fn only_the_server_to_server_endpoints_are_pointed_at_the_discovery_host() {
        let issuer = "https://public.example.org";

        let mut config = Config::default();
        config.oidc.issuer = Some(Url::from_str(issuer).unwrap());
        let oidc = Oidc::new(&config);

        let body = json!({
            "issuer": issuer,
            "authorization_endpoint": format!("{}/auth", issuer),
            "end_session_endpoint": format!("{}/logout", issuer),
            "token_endpoint": format!("{}/token", issuer),
            "jwks_uri": format!("{}/jwks", issuer),
            "userinfo_endpoint": format!("{}/userinfo", issuer),
        }).to_string();

        let rewritten: serde_json::Value = serde_json::from_str(
            &oidc.rewrite_server_endpoints(body, "http://keycloak:8080/metadata").unwrap()
        ).unwrap();

        // the app talks to these itself and has to reach them over the internal name
        for field in ["token_endpoint", "jwks_uri", "userinfo_endpoint"] {
            assert_eq!(
                rewritten[field].as_str().unwrap(),
                format!("http://keycloak:8080/{}", field.trim_end_matches("_endpoint").trim_end_matches("_uri")),
                "{} was not pointed at the discovery host", field,
            );
        }

        // the browser is sent to these, and it only knows the public name
        assert_eq!(rewritten["authorization_endpoint"], format!("{}/auth", issuer));
        assert_eq!(rewritten["end_session_endpoint"], format!("{}/logout", issuer));
        // the issuer is what tokens are trusted against and must not move
        assert_eq!(rewritten["issuer"], issuer);
    }
}
