use super::{convert::IntoMastodon, Authorisation};
use crate::{
    activitypub::interactions,
    database::{Actor as DbActor, Object as DbObject},
    error::Error,
    format_uuid,
    state::ArcState, util::Form, consts::MAX_BODY_SIZE,
};
use axum::{
    extract::{Path, ContentLengthLimit},
    response::IntoResponse,
    routing::{get, post, patch},
    Extension, Json, Router,
};
use serde::Deserialize;
use tranquility_types::{
    activitypub::{Actor, Tag, Attachment},
    mastodon::{Account, FollowResponse, Source},
};
use uuid::Uuid;

async fn accounts(
    Path(id): Path<Uuid>,
    Extension(state): Extension<ArcState>,
    authorized_db_actor: Option<Authorisation>,
) -> Result<impl IntoResponse, Error> {
    let db_actor = DbActor::get(&state.db_pool, id).await?;
    let mut mastodon_account: Account = db_actor.into_mastodon(&state).await?;

    // Add the source field to the returned account if the requested account
    // is the account that has authorized itself
    if let Some(Authorisation(authorized_db_actor)) = authorized_db_actor {
        if id == authorized_db_actor.id {
            let source: Source = authorized_db_actor.into_mastodon(&state).await?;
            mastodon_account.source = Some(source);
        }
    }

    Ok(Json(mastodon_account))
}

async fn follow(
    Path(id): Path<Uuid>,
    Extension(state): Extension<ArcState>,
    Authorisation(authorized_db_actor): Authorisation,
) -> Result<impl IntoResponse, Error> {
    let followed_db_actor = DbActor::get(&state.db_pool, id).await?;
    let followed_actor: Actor = serde_json::from_value(followed_db_actor.actor)?;

    interactions::follow(&state, authorized_db_actor, &followed_actor).await?;

    // TODO: Fill in information dynamically (followed by, blocked by, blocking, etc.)
    let follow_response = FollowResponse {
        id: format_uuid!(followed_db_actor.id),
        following: true,
        ..FollowResponse::default()
    };
    Ok(Json(follow_response))
}

async fn following(
    Path(id): Path<Uuid>,
    Extension(state): Extension<ArcState>,
) -> Result<impl IntoResponse, Error> {
    let follow_activities =
        DbObject::by_type_and_owner(&state.db_pool, "Follow", &id, 10, 0).await?;
    let followed_accounts: Vec<Account> = follow_activities.into_mastodon(&state).await?;

    Ok(Json(followed_accounts))
}

async fn followers(
    Path(id): Path<Uuid>,
    Extension(state): Extension<ArcState>,
) -> Result<impl IntoResponse, Error> {
    let db_actor = DbActor::get(&state.db_pool, id).await?;
    let actor: Actor = serde_json::from_value(db_actor.actor)?;

    let followed_activities =
        DbObject::by_type_and_object_url(&state.db_pool, "Follow", actor.id.as_str(), 10, 0)
            .await?;
    let follower_accounts: Vec<Account> = followed_activities.into_mastodon(&state).await?;

    Ok(Json(follower_accounts))
}

// TODO: Implement `/api/v1/accounts/:id/statuses` endpoint
/*async fn statuses(Path(id): Path<Uuid>, authorized_db_actor: Option<Auth>) -> Result<impl Reply, Rejection> {
}*/

async fn unfollow(
    Path(id): Path<Uuid>,
    Extension(state): Extension<ArcState>,
    Authorisation(authorized_db_actor): Authorisation,
) -> Result<impl IntoResponse, Error> {
    // Fetch the follow activity
    let followed_db_actor = DbActor::get(&state.db_pool, id).await?;
    let followed_actor_id = format_uuid!(followed_db_actor.id);

    interactions::unfollow(&state, authorized_db_actor, followed_db_actor).await?;

    // TODO: Fill in information dynamically (followed by, blocked by, blocking, etc.)
    let unfollow_response = FollowResponse {
        id: followed_actor_id,
        ..FollowResponse::default()
    };
    Ok(Json(unfollow_response))
}

async fn verify_credentials(
    Extension(state): Extension<ArcState>,
    Authorisation(db_actor): Authorisation,
) -> Result<impl IntoResponse, Error> {
    let mut mastodon_account: Account = db_actor.clone().into_mastodon(&state).await?;
    let mastodon_account_source: Source = db_actor.into_mastodon(&state).await?;

    mastodon_account.source = Some(mastodon_account_source);

    Ok(Json(mastodon_account))
}

#[derive(Deserialize)]
pub struct UpdateForm {
    name: Option<String>,
    summary: Option<String>,
    tag: Option<Vec<Tag>>,
    icon: Option<Attachment>,
    image: Option<Attachment>,
    manually_approves_followers: Option<bool>,
}

async fn update_credentials(
    Extension(state): Extension<ArcState>,
    Authorisation(authorized_db_actor): Authorisation,
    ContentLengthLimit(Form(form)): ContentLengthLimit<Form<UpdateForm>, MAX_BODY_SIZE>,
) -> Result<impl IntoResponse, Error> {
  let mut db_actor = DbActor::get(&state.db_pool, authorized_db_actor.id).await?;
  let mut actor: Actor = serde_json::from_value(db_actor.actor)?;
  if let Some(name) = form.name {
    actor.name = name;
  }
  if let Some(summary) = form.summary {
    actor.summary = summary;
  }
  if let Some(tag) = form.tag {
    actor.tag = tag;
  }
  if let Some(icon) = form.icon {
    actor.icon = Some(icon);
  }
  if let Some(image) = form.image {
    actor.image = Some(image);
  }
  if let Some(manually_approves_followers) = form.manually_approves_followers {
    actor.manually_approves_followers = manually_approves_followers;
  }

  db_actor.actor = serde_json::to_value(actor)?;

  DbActor::update(&state.db_pool, &db_actor).await?;

  let mastodon_account: Account = db_actor.clone().into_mastodon(&state).await?;

  Ok(Json(mastodon_account))
}

pub fn routes() -> Router {
    Router::new()
        .route("/accounts/:id", get(accounts))
        .route("/accounts/:id/follow", post(follow))
        .route("/accounts/:id/following", get(following))
        .route("/accounts/:id/followers", get(followers))
        //.route("/accounts/:id/statuses", get(statuses))
        .route("/accounts/:id/unfollow", post(unfollow))
        .route("/accounts/verify_credentials", get(verify_credentials))
        .route("/accounts/update_credentials", patch(update_credentials))
}
