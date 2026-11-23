use super::*;

#[test]
fn instance_is_80_bytes() {
    assert_eq!(std::mem::size_of::<Instance>(), 80);
}

#[test]
fn batches_group_by_mesh() {
    let inst = |k: f32| Instance { model: [[0.0; 4]; 4], color: [k, 0.0, 0.0, 1.0] };
    let draws: Vec<Draw> = [(7, 1.0, true), (3, 2.0, true), (7, 3.0, false), (7, 4.0, true)]
        .into_iter()
        .map(|(mesh, k, faces)| Draw { mesh, instance: inst(k), faces })
        .collect();
    let (instances, batches) = batch(&draws);
    assert_eq!(instances.iter().map(|i| i.color[0]).collect::<Vec<_>>(), [2.0, 1.0, 4.0, 3.0]);
    assert_eq!(
        batches,
        [
            Batch { mesh: 3, range: 0..1, faces: true },
            Batch { mesh: 7, range: 1..3, faces: true },
            Batch { mesh: 7, range: 3..4, faces: false },
        ]
    );
    let (instances, batches) = batch(&[]);
    assert!(instances.is_empty() && batches.is_empty());
}
