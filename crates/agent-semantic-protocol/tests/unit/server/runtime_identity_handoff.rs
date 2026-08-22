use super::{RuntimeIdentityHandoff, execute_runtime_identity_handoff};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct Fake {
    events: Arc<Mutex<Vec<&'static str>>>,
    fail: bool,
}
impl RuntimeIdentityHandoff for Fake {
    fn retire(
        &mut self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + '_>> {
        self.events.lock().unwrap().push("retire");
        Box::pin(async { Ok(()) })
    }
    fn cleanup(
        &mut self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + '_>> {
        self.events.lock().unwrap().push("cleanup");
        Box::pin(async { Ok(()) })
    }
    fn admit(
        &mut self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + '_>> {
        let fail = self.fail;
        self.events.lock().unwrap().push("admit");
        Box::pin(async move {
            if fail {
                Err("stale-epoch".into())
            } else {
                Ok(())
            }
        })
    }
    fn publish(
        &mut self,
        success: bool,
        _: Option<String>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + '_>> {
        self.events.lock().unwrap().push(if success {
            "publish-success"
        } else {
            "publish-failure"
        });
        Box::pin(async {})
    }
}

#[tokio::test]
async fn identity_handoff_publishes_successor_healthy_endpoint() {
    let f = Fake {
        events: Arc::new(Mutex::new(Vec::new())),
        fail: false,
    };
    execute_runtime_identity_handoff(f.clone()).await.unwrap();
    assert_eq!(
        &*f.events.lock().unwrap(),
        &["retire", "cleanup", "admit", "publish-success"]
    );
}
#[tokio::test]
async fn identity_handoff_readiness_failure_publishes_typed_failure() {
    let f = Fake {
        events: Arc::new(Mutex::new(Vec::new())),
        fail: true,
    };
    let e = execute_runtime_identity_handoff(f.clone())
        .await
        .unwrap_err();
    assert!(e.contains("successor failed readiness admission"));
    assert_eq!(
        &*f.events.lock().unwrap(),
        &["retire", "cleanup", "admit", "publish-failure"]
    );
}
#[tokio::test]
async fn stale_identity_successor_cannot_publish_over_newer_epoch() {
    let f = Fake {
        events: Arc::new(Mutex::new(Vec::new())),
        fail: true,
    };
    assert!(execute_runtime_identity_handoff(f.clone()).await.is_err());
    assert!(!f.events.lock().unwrap().contains(&"publish-success"));
}
#[tokio::test]
async fn identity_handoff_never_leaves_stopped_without_endpoint_gap() {
    let f = Fake {
        events: Arc::new(Mutex::new(Vec::new())),
        fail: false,
    };
    execute_runtime_identity_handoff(f.clone()).await.unwrap();
    assert_eq!(f.events.lock().unwrap().last(), Some(&"publish-success"));
}
