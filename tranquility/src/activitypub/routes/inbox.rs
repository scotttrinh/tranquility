use {
    crate::{
        activitypub::{
            fetcher, handler,
            routes::{custom_json_parser, optional_raw_query},
        },
        config::ArcConfig,
        crypto,
        error::Error,
    },
    core::ops::Not,
    tranquility_types::activitypub::{activity::ObjectField, Activity},
    warp::{
        http::{HeaderMap, Method},
        path::FullPath,
        Filter, Rejection, Reply,
    },
};

pub fn validate_request() -> impl Filter<Extract = (Activity,), Error = Rejection> + Copy {
    warp::method()
        .and(warp::path::full())
        .and(optional_raw_query())
        .and(warp::header::headers_cloned())
        .and(custom_json_parser())
        .and_then(verify_signature)
        .and_then(verify_ownership)
}

async fn verify_ownership(activity: Activity) -> Result<Activity, Rejection> {
    // It's fine if the objects or activities don't match in this case
    if activity.r#type == "Announce" || activity.r#type == "Follow" {
        return Ok(activity);
    }

    let identity_match = match activity.object {
        ObjectField::Actor(ref actor) => actor.id == activity.actor,
        ObjectField::Object(ref object) => object.attributed_to == activity.actor,
        ObjectField::Url(ref url) => {
            let entity = fetcher::fetch_any(url).await?;
            entity.is_owned_by(activity.actor.as_str())
        }
    };

    identity_match
        .then(|| activity)
        .ok_or_else(|| Error::Unauthorized.into())
}

async fn verify_signature(
    method: Method,
    path: FullPath,
    query: String,
    headers: HeaderMap,
    activity: Activity,
) -> Result<Activity, Rejection> {
    let (remote_actor, _remote_actor_db) = fetcher::fetch_actor(activity.actor.as_ref())
        .await
        .map_err(Error::from)?;

    let public_key = remote_actor.public_key.public_key_pem;
    let query = query.is_empty().not().then(|| query);

    crypto::request::verify(method, path, query, headers, public_key)
        .await?
        .then(|| activity)
        .ok_or_else(|| Error::Unauthorized.into())
}

pub async fn inbox(
    // Do we even care about the user ID?
    // Theoretically we could just use one shared inbox and get rid of the unique inboxes
    _user_id: uuid::Uuid,
    config: ArcConfig,
    activity: Activity,
) -> Result<impl Reply, Rejection> {
    let response = match activity.r#type.as_str() {
        "Accept" => handler::accept::handle(activity).await,
        "Create" => handler::create::handle(activity).await,
        "Delete" => handler::delete::handle(activity).await,
        "Follow" => handler::follow::handle(&config, activity).await,
        "Like" => handler::like::handle(activity).await,
        "Reject" => handler::reject::handle(activity).await,
        "Undo" => handler::undo::handle(activity).await,
        "Update" => handler::update::handle(activity).await,
        _ => Err(Error::UnknownActivity),
    };

    response.map_err(Rejection::from)
}
