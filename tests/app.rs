use catapulta::App;

#[test]
fn defaults() {
    let app = App::new("myapp");

    assert_eq!(app.name, "myapp");
    assert_eq!(app.dockerfile, "Dockerfile");
    assert_eq!(app.platform, "linux/amd64");
    assert!(app.build_args.is_empty());
    assert!(app.env.is_empty());
    assert!(app.env_file.is_none());
    assert!(app.volumes.is_empty());
    assert!(app.expose.is_empty());
    assert!(app.healthcheck.is_none());
}

#[test]
fn builder_chain() {
    let app = App::new("test")
        .dockerfile("deploy/Dockerfile")
        .platform("linux/arm64")
        .build_arg("RUST_VERSION", "1.93.0")
        .build_arg("NODE_VERSION", "24")
        .env("HOST", "0.0.0.0")
        .env("PORT", "3000")
        .env_file(".env")
        .volume("data", "/app/data")
        .volume("config", "/app/config")
        .expose(3000)
        .expose(8080)
        .healthcheck("curl -f http://localhost:3000/");

    assert_eq!(app.dockerfile, "deploy/Dockerfile");
    assert_eq!(app.platform, "linux/arm64");
    assert_eq!(
        app.build_args,
        vec![
            ("RUST_VERSION".into(), "1.93.0".into()),
            ("NODE_VERSION".into(), "24".into()),
        ]
    );
    assert_eq!(
        app.env,
        vec![
            ("HOST".into(), "0.0.0.0".into()),
            ("PORT".into(), "3000".into()),
        ]
    );
    assert_eq!(app.env_file.as_deref(), Some(".env"));
    assert_eq!(
        app.volumes,
        vec![
            ("data".into(), "/app/data".into()),
            ("config".into(), "/app/config".into()),
        ]
    );
    assert_eq!(app.expose, vec![3000, 8080]);
    assert_eq!(
        app.healthcheck.as_deref(),
        Some("curl -f http://localhost:3000/")
    );
}

#[test]
fn env_file_overrides() {
    let app = App::new("x").env_file("first.env").env_file("second.env");

    assert_eq!(app.env_file.as_deref(), Some("second.env"));
}
