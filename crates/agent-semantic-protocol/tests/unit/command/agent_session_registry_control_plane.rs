#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ControlPlaneStep {
    HookDenied,
    ResumeChecked,
    StatusChecked,
    ModelGuidanceObserved,
    ChildMessageAttempted,
    ReceiptReturned,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ControlPlaneVerdict {
    Complete,
    NeedsModelGuidance,
    NeedsChildMessageTarget,
    NeedsBoundedReceipt,
    NeedsArchive,
}

fn classify_normal_thread_control_plane(steps: &[ControlPlaneStep]) -> ControlPlaneVerdict {
    if !steps.contains(&ControlPlaneStep::ModelGuidanceObserved) {
        return ControlPlaneVerdict::NeedsModelGuidance;
    }
    if !steps.contains(&ControlPlaneStep::ChildMessageAttempted) {
        return ControlPlaneVerdict::NeedsChildMessageTarget;
    }
    if !steps.contains(&ControlPlaneStep::ReceiptReturned) {
        return ControlPlaneVerdict::NeedsBoundedReceipt;
    }
    if !steps.contains(&ControlPlaneStep::Archived) {
        return ControlPlaneVerdict::NeedsArchive;
    }
    ControlPlaneVerdict::Complete
}

#[test]
fn control_plane_requires_model_guidance_before_child_actions() {
    let verdict = classify_normal_thread_control_plane(&[
        ControlPlaneStep::HookDenied,
        ControlPlaneStep::ResumeChecked,
        ControlPlaneStep::StatusChecked,
    ]);

    assert_eq!(verdict, ControlPlaneVerdict::NeedsModelGuidance);
}

#[test]
fn control_plane_requires_native_child_message_target_after_guidance() {
    let verdict = classify_normal_thread_control_plane(&[
        ControlPlaneStep::HookDenied,
        ControlPlaneStep::ResumeChecked,
        ControlPlaneStep::StatusChecked,
        ControlPlaneStep::ModelGuidanceObserved,
    ]);

    assert_eq!(verdict, ControlPlaneVerdict::NeedsChildMessageTarget);
}

#[test]
fn control_plane_requires_bounded_receipt_after_child_message_attempt() {
    let verdict = classify_normal_thread_control_plane(&[
        ControlPlaneStep::HookDenied,
        ControlPlaneStep::ResumeChecked,
        ControlPlaneStep::StatusChecked,
        ControlPlaneStep::ModelGuidanceObserved,
        ControlPlaneStep::ChildMessageAttempted,
    ]);

    assert_eq!(verdict, ControlPlaneVerdict::NeedsBoundedReceipt);
}

#[test]
fn control_plane_requires_archive_after_receipt() {
    let verdict = classify_normal_thread_control_plane(&[
        ControlPlaneStep::HookDenied,
        ControlPlaneStep::ResumeChecked,
        ControlPlaneStep::StatusChecked,
        ControlPlaneStep::ModelGuidanceObserved,
        ControlPlaneStep::ChildMessageAttempted,
        ControlPlaneStep::ReceiptReturned,
    ]);

    assert_eq!(verdict, ControlPlaneVerdict::NeedsArchive);
}

#[test]
fn control_plane_completes_only_after_archive() {
    let verdict = classify_normal_thread_control_plane(&[
        ControlPlaneStep::HookDenied,
        ControlPlaneStep::ResumeChecked,
        ControlPlaneStep::StatusChecked,
        ControlPlaneStep::ModelGuidanceObserved,
        ControlPlaneStep::ChildMessageAttempted,
        ControlPlaneStep::ReceiptReturned,
        ControlPlaneStep::Archived,
    ]);

    assert_eq!(verdict, ControlPlaneVerdict::Complete);
}
