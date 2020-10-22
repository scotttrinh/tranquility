use {crate::error::Error, tranquility_types::activitypub::Activity, warp::http::StatusCode};

pub async fn handle(delete_activity: Activity) -> Result<StatusCode, Error> {
    let activity_url = delete_activity.object.as_url().ok_or(Error::FetchError)?;

    let activity = crate::database::activity::select::by_url(activity_url.clone()).await?;
    let activity: Activity = serde_json::from_value(activity.data)?;

    // Does the activity belong to the actor?
    if delete_activity.actor != activity.actor {
        return Err(Error::Unauthorized);
    }

    crate::database::activity::delete::by_url(activity.id).await?;

    Ok(StatusCode::CREATED)
}
