//! The Postgres store against a real database made by deploy/doodlessd/db-init/01-roles.sh.
//! Skipped unless FOLIOLUM_TEST_OWNER_URL and FOLIOLUM_TEST_APP_URL are set (see README.md).
use foliolum_api::pg::{Pg, migrate};
use foliolum_api::store::{InviteError, Role, Store};

fn urls() -> Option<(String, String)> {
    Some((std::env::var("FOLIOLUM_TEST_OWNER_URL").ok()?, std::env::var("FOLIOLUM_TEST_APP_URL").ok()?))
}

#[tokio::test]
async fn the_postgres_store_keeps_parishes_and_templates() {
    let Some((owner, app_url)) = urls() else {
        eprintln!("skipped: no FOLIOLUM_TEST_OWNER_URL / FOLIOLUM_TEST_APP_URL");
        return;
    };
    let owner_config: tokio_postgres::Config = owner.parse().unwrap();
    migrate(&owner_config).await.unwrap();
    migrate(&owner_config).await.unwrap(); // applying twice is a no-op
    let s = Pg::new(app_url.parse().unwrap()).unwrap();
    assert!(s.ready().await);
    let tag = uuid::Uuid::new_v4().simple().to_string();
    let (a, b) = (format!("a-{tag}"), format!("b-{tag}"));
    s.upsert_user(&a, "a@example.org", "Ann").await.unwrap();
    s.upsert_user(&b, "b@example.org", "Bea").await.unwrap();
    s.upsert_user(&a, "a@example.org", "Ann B").await.unwrap();
    let p = s.create_parish(&a, "St Brigid's").await.unwrap();
    assert_eq!(s.parishes_created(&a).await.unwrap(), 1);
    assert_eq!(s.role(&a, p.id).await.unwrap(), Some(Role::Admin));
    assert_eq!(s.role(&b, p.id).await.unwrap(), None);

    let now = 1_900_000_000;
    s.create_invite(p.id, &a, [7; 32], now + 100, 1).await.unwrap();
    assert!(matches!(s.accept_invite([7; 32], &b, now + 200).await, Err(InviteError::Gone)), "expired");
    assert_eq!(s.accept_invite([7; 32], &b, now).await.unwrap().role, Role::Member);
    assert!(matches!(s.accept_invite([7; 32], &a, now).await, Err(InviteError::Gone)), "used up");
    assert_eq!(s.memberships(&b).await.unwrap()[0].name, "St Brigid's");

    let id = s.put_template(p.id, &b, "Bulletin", b"one", now).await.unwrap();
    assert_eq!(s.put_template(p.id, &a, "Bulletin", b"two!", now + 5).await.unwrap(), id, "same name replaces");
    let list = s.templates(p.id).await.unwrap();
    assert_eq!((list.len(), list[0].saved_by.as_str(), list[0].size, list[0].updated), (1, "Ann B", 4, now + 5));
    let t = s.template(id).await.unwrap().unwrap();
    assert_eq!((t.parish, t.document.as_slice(), t.saved_by_sub.as_str()), (p.id, b"two!".as_slice(), a.as_str()));
    s.delete_template(id).await.unwrap();
    assert!(s.template(id).await.unwrap().is_none());
    s.leave(&b, p.id).await.unwrap();
    assert!(s.memberships(&b).await.unwrap().is_empty());

    // The service's role reads and writes rows, nothing more.
    let (client, conn) = tokio_postgres::connect(&app_url, tokio_postgres::NoTls).await.unwrap();
    tokio::spawn(conn);
    assert!(client.batch_execute("create table nope (x int)").await.is_err());
    assert!(client.batch_execute("drop table templates").await.is_err());
}
