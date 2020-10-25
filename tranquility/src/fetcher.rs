use {
    crate::{database::model::Actor as DBActor, error::Error},
    reqwest::IntoUrl,
    serde_json::Value,
    tranquility_types::activitypub::{Activity, Actor, Object},
};

pub enum Entity {
    Activity(Activity),
    Actor(Actor),
    Object(Object),
}

impl Entity {
    pub fn into_activity(self) -> Option<Activity> {
        match self {
            Self::Activity(activity) => Some(activity),
            _ => None,
        }
    }

    pub fn into_actor(self) -> Option<Actor> {
        match self {
            Self::Actor(actor) => Some(actor),
            _ => None,
        }
    }

    pub fn into_object(self) -> Option<Object> {
        match self {
            Self::Object(object) => Some(object),
            _ => None,
        }
    }
}

pub async fn fetch_activity(url: &str) -> Result<Activity, Error> {
    match crate::database::object::select::by_url(url).await {
        Ok(activity) => return Ok(serde_json::from_value(activity.data)?),
        Err(e) => {
            debug!("{}", e);
            debug!("Activity not found in database. Attempting remote fetch...");
        }
    }

    if let Entity::Activity(activity) = fetch_entity(url).await? {
        let (actor, _actor_db) = fetch_actor(activity.actor.as_ref()).await?;
        let actor = crate::database::actor::select::by_url(actor.id.as_ref()).await?;

        let activity_value = serde_json::to_value(&activity)?;
        crate::database::object::insert(actor.id, &activity.id, activity_value).await?;

        Ok(activity)
    } else {
        debug!("Remote server returned content we can't interpret");

        Err(Error::Fetch)
    }
}

pub async fn fetch_actor(url: &str) -> Result<(Actor, DBActor), Error> {
    match crate::database::actor::select::by_url(url).await {
        Ok(actor) => return Ok((serde_json::from_value(actor.actor.clone())?, actor)),
        Err(e) => {
            debug!("{}", e);
            debug!("Actor not found in database. Attempting remote fetch...");
        }
    }

    if let Entity::Actor(actor) = fetch_entity(url).await? {
        let db_actor =
            crate::database::actor::insert::remote(actor.username.as_ref(), &actor).await?;

        Ok((actor, db_actor))
    } else {
        debug!("Remote server returned content we can't interpret");

        Err(Error::Fetch)
    }
}

pub async fn fetch_object(url: &str) -> Result<Object, Error> {
    match crate::database::object::select::by_url(url).await {
        Ok(object) => return Ok(serde_json::from_value(object.data)?),
        Err(e) => {
            debug!("{}", e);
            debug!("Object not found in database. Attempting remote fetch...");
        }
    }

    if let Entity::Object(object) = fetch_entity(url).await? {
        let (actor, _actor_db) = fetch_actor(object.attributed_to.as_ref()).await?;
        let actor = crate::database::actor::select::by_url(actor.id.as_ref()).await?;

        let object_value = serde_json::to_value(&object)?;
        crate::database::object::insert(actor.id, &object.id, object_value).await?;

        Ok(object)
    } else {
        debug!("Remote server returned content we can't interpret");

        Err(Error::Fetch)
    }
}

async fn fetch_entity<T: IntoUrl + Send>(url: T) -> Result<Entity, Error> {
    let client = &crate::REQWEST_CLIENT;
    let request = client
        .get(url)
        .header(
            "Accept",
            "application/ld+json; profile=\"https://www.w3.org/ns/activitystreams\"",
        )
        .build()?;

    let entity: Value = client.execute(request).await?.json().await?;

    let entity = if entity["type"].as_str().unwrap() == "Person" {
        // This should be deserializable into an actor
        let actor = serde_json::from_value(entity)?;

        Entity::Actor(actor)
    } else if entity.get("object").is_some() {
        // This should be deserializable into an activity
        let activity = serde_json::from_value(entity)?;

        Entity::Activity(activity)
    } else {
        // This could be deserializable into an object (but could also be nothing)
        let object = serde_json::from_value(entity)?;

        Entity::Object(object)
    };

    Ok(entity)
}
