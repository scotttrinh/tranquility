use {crate::error::Error, tranquility_types::activitypub::Activity, warp::http::StatusCode};

pub async fn handle(activity: Activity) -> Result<StatusCode, Error> {
    let activity_url = activity.object.as_url().ok_or(Error::UnknownActivity)?;

    // Fetch the activity (just in case)
    crate::fetcher::fetch_activity(activity_url.clone()).await?;
    // Fetch the actor (just in case)
    crate::fetcher::fetch_actor(activity.actor.clone()).await?;
    let actor = crate::database::actor::select::by_url(activity.actor.clone()).await?;

    crate::database::activity::insert(actor.id, activity).await?;

    Ok(StatusCode::CREATED)
}
