use crate::domain::{self, UserId};
use anyhow::anyhow;
use tide::{http::headers::AUTHORIZATION, Request, Response, StatusCode};

pub async fn secret(req: Request<impl domain::db::Db>) -> tide::Result {
    let user = UserId(req.param("user")?.to_string());

    if domain::can_access_secret(req.state(), &user)? {
        Ok(Response::builder(StatusCode::Ok)
            .body(format!("Secrets for user {:?}", user))
            .build())
    } else {
        Err(tide::Error::new(
            StatusCode::Forbidden,
            anyhow!("Not allowed"),
        ))
    }
}

pub async fn login(req: Request<impl domain::db::Db>) -> tide::Result {
    if let Some(auth) = req.header(AUTHORIZATION) {
        domain::login(req.state(), auth.as_str())?;
    }

    Ok(Response::new(StatusCode::Ok))
}

pub async fn logout(req: Request<impl domain::db::Db>) -> tide::Result {
    if let Some(auth) = req.header(AUTHORIZATION) {
        domain::logout(req.state(), auth.as_str())?;
    }
    Ok(Response::new(StatusCode::Ok))
}
