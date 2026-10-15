//! APFS volumes: the instant top level of the storage detail view. macOS
//! itself, its support volumes and the Data volume, read from APFS, which
//! already knows each volume's size: no walking needed.

use alpheus_lib::apfs;

/// `diskutil apfs list -plist disk3 | plutil -convert json`, trimmed.
const LIST: &str = r#"{"Containers":[{
  "ContainerReference":"disk3","CapacityCeiling":245107195904,"CapacityFree":29686919168,
  "Volumes":[
    {"DeviceIdentifier":"disk3s1","Name":"Macintosh HD","Roles":["System"],"CapacityInUse":18350000128},
    {"DeviceIdentifier":"disk3s2","Name":"Preboot","Roles":["Preboot"],"CapacityInUse":21830000640},
    {"DeviceIdentifier":"disk3s3","Name":"Recovery","Roles":["Recovery"],"CapacityInUse":3040000000},
    {"DeviceIdentifier":"disk3s4","Name":"Update","Roles":["Update"],"CapacityInUse":1240000000},
    {"DeviceIdentifier":"disk3s5","Name":"Data","Roles":["Data"],"CapacityInUse":195130000384},
    {"DeviceIdentifier":"disk3s6","Name":"VM","Roles":["VM"],"CapacityInUse":1070000128},
    {"DeviceIdentifier":"disk3s7","Name":"Scratch","Roles":[],"CapacityInUse":1048576}
  ]}]}"#;

#[test]
fn test_parses_every_volume_with_role_and_size() {
    let c = apfs::parse_container(LIST).expect("fixture must parse");
    assert_eq!(c.reference, "disk3");
    assert_eq!(c.capacity_kb, 245107195904 / 1024);
    assert_eq!(c.free_kb, 29686919168 / 1024);
    assert_eq!(c.volumes.len(), 7);

    let data = c.role("Data").expect("Data volume");
    assert_eq!(data.name, "Data");
    assert_eq!(data.used_kb, 195130000384 / 1024);
    assert_eq!(c.role("System").unwrap().name, "Macintosh HD");
    assert!(c.role("Backup").is_none());

    // A volume with no role is still a volume the user made: keep it.
    let scratch = c.volumes.iter().find(|v| v.name == "Scratch").unwrap();
    assert!(scratch.roles.is_empty());
}

#[test]
fn test_garbage_is_none_not_a_panic() {
    assert!(apfs::parse_container("").is_none());
    assert!(apfs::parse_container("{ not json").is_none());
    assert!(apfs::parse_container(r#"{"Containers":[]}"#).is_none());
    assert!(apfs::parse_container(r#"{"Something":"else"}"#).is_none());
}

/// On a real Mac: the startup disk's container is found, has macOS and Data,
/// and its volumes plus free space account for its capacity.
#[cfg(target_os = "macos")]
#[test]
fn test_reads_the_startup_container() {
    let c = apfs::read_root().expect("the startup disk is APFS");
    assert!(c.role("System").is_some(), "no System volume");
    assert!(c.role("Data").is_some(), "no Data volume");
    let accounted: u64 = c.volumes.iter().map(|v| v.used_kb).sum::<u64>() + c.free_kb;
    let gap = c.capacity_kb.abs_diff(accounted) as f64 / c.capacity_kb as f64;
    assert!(
        gap < 0.01,
        "volumes + free miss capacity by {:.2}%",
        gap * 100.0
    );
}
