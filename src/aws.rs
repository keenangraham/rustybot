use rusoto_core::Region;
use rusoto_ec2::Ec2;
use rusoto_ec2::Ec2Client;
use rusoto_ec2::DescribeInstancesRequest;
use rusoto_ec2::StopInstancesRequest;
use rusoto_ec2::StartInstancesRequest;
use rusoto_ec2::InstanceStateChange;
use rusoto_ec2::ModifyInstanceAttributeRequest;
use rusoto_ec2::AttributeValue;
use rusoto_ec2::filter;
use rusoto_ec2::Filter;
use rusoto_ec2::Reservation;
use rusoto_ec2::Instance;
use std::error::Error;
use regex::Regex;
use std::fmt;


#[derive(Debug, Eq, PartialEq)]
pub struct InstanceInfo {
    size: String,
    id: String,
    keyname: Option<String>,
    state: String,
    tags: Vec<(String, String)>
}


fn flatten_reservations(reservations: Vec<Reservation>) -> Vec<Instance> {
    let mut matching_instances: Vec<Instance> = vec![];
    for reservation in reservations {
	if let Some(instances) = reservation.instances {
	    matching_instances.extend(instances);
	}
    }
    matching_instances
}


fn make_describe_instances_request_with_filters(filters: Vec<Filter>) -> DescribeInstancesRequest {
    DescribeInstancesRequest {
	filters: Some(filters),
	..Default::default()
    }
}


fn make_stop_instances_request(instance_ids: Vec<String>) -> StopInstancesRequest {
    StopInstancesRequest {
	instance_ids: instance_ids,
	..Default::default()
    }
}

fn make_start_instances_request(instance_ids: Vec<String>) -> StartInstancesRequest {
    StartInstancesRequest {
	instance_ids: instance_ids,
	..Default::default()
    }
}

fn make_modify_instance_type_request(instance_id: String, size: String) -> ModifyInstanceAttributeRequest {
    ModifyInstanceAttributeRequest {
	instance_id,
	instance_type: Some(
	    AttributeValue{
		value: Some(size)
	    }
	),
	..Default::default()
    }
}


pub fn make_ec2_client() -> Ec2Client {
    Ec2Client::new(Region::UsWest2)
}


#[tokio::main]
async fn describe_instances_and_unwrap_reservations(ec2: &Ec2Client, request: DescribeInstancesRequest) -> Result<Vec<Reservation>, Box<dyn Error>> {
    let reservations = ec2
	.describe_instances(request)
	.await?
	.reservations
	.ok_or("No reservations")?;
    Ok(reservations)
}


#[tokio::main]
async fn stop_instances_and_unwrap_stopped_instances(ec2: &Ec2Client, request: StopInstancesRequest) -> Result<Vec<InstanceStateChange>, Box<dyn Error>> {
    let stopped_instances = ec2
	.stop_instances(request)
	.await?
	.stopping_instances
	.ok_or("No instances stopped")?;
    Ok(stopped_instances)
}


#[tokio::main]
async fn start_instances_and_unwrap_started_instances(ec2: &Ec2Client, request: StartInstancesRequest) -> Result<Vec<InstanceStateChange>, Box<dyn Error>> {
    let started_instances = ec2
	.start_instances(request)
	.await?
	.starting_instances
	.ok_or("No instances started")?;
    Ok(started_instances)
}


#[tokio::main]
async fn modify_instance_attribute(ec2: &Ec2Client, request: ModifyInstanceAttributeRequest) -> Result<(), Box<dyn Error>> {
    let modified_instance = ec2.modify_instance_attribute(request).await?;
    Ok(modified_instance)
}


fn get_instances_by_filters(ec2: &Ec2Client, filters: Vec<Filter>) -> Result<Vec<Instance>, Box<dyn Error>> {
    let request = make_describe_instances_request_with_filters(filters);
    let reservations = describe_instances_and_unwrap_reservations(ec2, request);
    let matching_instances = flatten_reservations(reservations?);
    Ok(matching_instances)
}


fn stop_instances_by_ids(ec2: &Ec2Client, instance_ids: Vec<String>) -> Result<Vec<InstanceStateChange>, Box<dyn Error>> {
    let request = make_stop_instances_request(instance_ids);
    stop_instances_and_unwrap_stopped_instances(ec2, request)
}


fn start_instances_by_ids(ec2: &Ec2Client, instance_ids: Vec<String>) -> Result<Vec<InstanceStateChange>, Box<dyn Error>> {
    let request = make_start_instances_request(instance_ids);
    start_instances_and_unwrap_started_instances(ec2, request)
}


fn resize_instance_by_id(ec2: &Ec2Client, instance_id: String, size: String) -> Result<(), Box<dyn Error>> {
    let request = make_modify_instance_type_request(instance_id, size);
    modify_instance_attribute(ec2, request)
}


fn get_info_from_instances(instances: Vec<Instance>) -> Vec<InstanceInfo> {
    let mut info: Vec<InstanceInfo> = vec![];
    for instance in instances {
	info.push(
	    InstanceInfo {
		size: instance.instance_type.unwrap(),
		id: instance.instance_id.unwrap(),
		keyname: instance.key_name,
		state: instance.state.unwrap().name.unwrap(),
		tags: instance.tags.unwrap().into_iter().map(
		    |x| (x.key.unwrap(), x.value.unwrap())
		).collect::<Vec<(String, String)>>()
	    }
	)
    }
    info
}


fn parse_name_from_url(url: String) -> Option<String> {
    let name_re: Regex = Regex::new(r"http[s]?://([a-zA-Z][-0-9a-zA-Z_]*)\.").unwrap();
    if let Some(capture) = name_re.captures(&url) {
	return Some(capture.get(1).unwrap().as_str().to_owned());
    }
    None
}


fn is_instance_id(value: String) -> bool {
    let instance_id_re: Regex = Regex::new(r"^i-[0-9][0-9a-zA-Z]*").unwrap();
    if let Some(capture) = instance_id_re.captures(&value) {
	return true;
    }
    false
}


fn get_instance_info_from_name(ec2: &Ec2Client, name: String) -> Vec<InstanceInfo> {
    let instances = get_instances_by_filters(
	ec2,
	vec![filter!("tag:Name", name)],
    ).unwrap();
    get_info_from_instances(instances)
}


fn get_instance_info_from_id(ec2: &Ec2Client, id: String) -> Vec<InstanceInfo> {
    let instances = get_instances_by_filters(
	ec2,
	vec![filter!("instance-id", id)],
    ).unwrap();
    get_info_from_instances(instances)
}


fn make_filters_from_tuples(filters: Vec<(String, String)>) -> Vec<Filter> {
    filters.iter().map(
	|f| filter!(f.0, f.1)
    ).collect::<Vec<Filter>>()
}


fn get_instance_ids_from_url_or_id(ec2: &Ec2Client, url_or_id: String) -> Vec<String>{
    let mut instance_ids = vec![];
    if let Some(name) = parse_name_from_url(url_or_id.clone()) {
	let instance_info = get_instance_info_from_name(ec2, name);
	instance_ids = instance_info.iter().map(
	    |x| x.id.to_owned()
	).collect::<Vec<String>>();
    } else if is_instance_id(url_or_id.clone()) {
	instance_ids = vec![url_or_id];
    }
    instance_ids
}


pub fn get_instance_info_from_filters(ec2: &Ec2Client, filters: Vec<(String, String)>) -> Result<Vec<InstanceInfo>, Box<dyn Error>> {
    let filters = make_filters_from_tuples(filters);
    let instances = get_instances_by_filters(
	&ec2,
	filters,
    )?;
    Ok(get_info_from_instances(instances))
}


pub fn get_instance_info_from_url_or_id(ec2: &Ec2Client, url_or_id: String) -> Vec<InstanceInfo> {
    let mut instance_info = vec![];
    if let Some(name) = parse_name_from_url(url_or_id.clone()) {
	instance_info = get_instance_info_from_name(ec2, name);
    } else if is_instance_id(url_or_id.clone()) {
	instance_info = get_instance_info_from_id(ec2, url_or_id.clone());
    }
    instance_info
}


pub fn stop_instance_by_url_or_id(ec2: &Ec2Client, url_or_id: String) -> Result<Vec<InstanceStateChange>, Box<dyn Error>> {
    let instance_ids = get_instance_ids_from_url_or_id(ec2, url_or_id);
    stop_instances_by_ids(ec2, instance_ids)
}


pub fn start_instance_by_url_or_id(ec2: &Ec2Client, url_or_id: String) -> Result<Vec<InstanceStateChange>, Box<dyn Error>> {
    let instance_ids = get_instance_ids_from_url_or_id(ec2, url_or_id);
    start_instances_by_ids(ec2, instance_ids)
}


pub fn resize_instance_by_url_or_id(ec2: &Ec2Client, url_or_id: String, size: String) -> Result<(), Box<dyn Error>> {
    if let Some(instance_id) = get_instance_ids_from_url_or_id(ec2, url_or_id).pop() {
	return resize_instance_by_id(ec2, instance_id.clone(), size.clone());
    }
    Err("No instances found".into())
}


#[cfg(test)]
mod tests {
    use super::*;
    use rusoto_mock::{
	MockCredentialsProvider,
	MockRequestDispatcher,
    };


    const DESCRIBE_INSTANCES_BODY: &str =
            r#"
            <?xml version="1.0" encoding="UTF-8"?><DescribeInstancesResponse xmlns="http://ec2.amazonaws.com/doc/2014-06-15/">
            <requestId>d15d204f-fc31-4600-85d3-5c86e5483b92</requestId><reservationSet><item><reservationId>r-9b4f3ca8</reservationId>
            <ownerId>123456789012</ownerId><groupSet><item><groupId>sg-4e970e7e</groupId><groupName>notebook</groupName></item></groupSet>
            <instancesSet><item><instanceId>i-0c3cbd3a6e1b8ffc8</instanceId><imageId>ami-30fe7300</imageId><instanceState><code>80</code>
            <name>stopped</name></instanceState><privateDnsName/><dnsName/><reason>User initiated (2013-03-01 17:24:16 GMT)</reason>
            <keyName>encoded-demos</keyName><amiLaunchIndex>0</amiLaunchIndex><productCodes/><instanceType>c5.9xlarge</instanceType>
            <launchTime>2012-10-16T20:00:13.000Z</launchTime><placement><availabilityZone>us-west-2</availabilityZone>
            <groupName/><tenancy>default</tenancy></placement><kernelId>aki-98e26fa8</kernelId><monitoring><state>running</state>
            </monitoring><groupSet><item><groupId>sg-4e970e7e</groupId><groupName>notebook</groupName></item></groupSet>
            <stateReason><code>Client.UserInitiatedShutdown</code><message>Client.UserInitiatedShutdown: User initiated shutdown</message>
            </stateReason><architecture>x86_64</architecture><rootDeviceType>ebs</rootDeviceType><rootDeviceName>/dev/sda1</rootDeviceName>
            <blockDeviceMapping><item><deviceName>/dev/sda1</deviceName><ebs><volumeId>vol-bc71579a</volumeId><status>attached</status>
            <attachTime>2012-10-16T20:00:21.000Z</attachTime><deleteOnTermination>true</deleteOnTermination></ebs></item></blockDeviceMapping>
            <virtualizationType>paravirtual</virtualizationType><clientToken/><tagSet><item><key>notebook</key><value>123</value></item>
            <item><key>branch</key><value>ENCD-5328-fix-released-start-date</value></item><item><key>commit</key><value>3a048a0ae</value></item>
            <item><key>started_by</key><value>emma</value></item><item><key>Name</key><value>encd-5328-3a048a0ae-emma</value></item></tagSet>
            <hypervisor>xen</hypervisor><networkInterfaceSet/><ebsOptimized>false</ebsOptimized></item></instancesSet></item>
            </reservationSet></DescribeInstancesResponse>
            "#;
    

    fn make_mock_ec2client(body: &str) -> Ec2Client {
	let mock = MockRequestDispatcher::default().with_body(body);
        Ec2Client::new_with(
	    mock,
	    MockCredentialsProvider,
	    Default::default()
	)
    }


    fn make_expected_describe_instances_request() -> DescribeInstancesRequest {
	DescribeInstancesRequest{
	    dry_run: None,
	    filters: Some(
		vec![
		    Filter{
			name: Some("instance-type".to_owned()),
			values: Some(vec!["t2.micro".to_owned()])
		    }
		]
	    ),
	    instance_ids: None,
	    max_results: None,
	    next_token: None
	}
    }


    fn make_expected_instance_info() -> InstanceInfo {
	InstanceInfo {
	    size: "c5.9xlarge".to_owned(),
	    id: "i-0c3cbd3a6e1b8ffc8".to_owned(),
	    keyname: Some("encoded-demos".to_owned()),
	    state: "stopped".to_owned(),
	    tags: vec![
		("notebook".to_owned(), "123".to_owned()),
		("branch".to_owned(), "ENCD-5328-fix-released-start-date".to_owned()),
		("commit".to_owned(), "3a048a0ae".to_owned()),
		("started_by".to_owned(), "emma".to_owned()),
		("Name".to_owned(), "encd-5328-3a048a0ae-emma".to_owned())
	    ]
	}
    }


    #[test]
    fn test_make_describe_instances_request_with_filters() {
	let actual = make_describe_instances_request_with_filters(
	    vec![filter!("instance-type", "t2.micro")]
	);
	let expected = make_expected_describe_instances_request();
        assert_eq!(actual, expected);
    }


    #[test]
    fn test_describe_instances_and_parse_info() {
	let ec2 = make_mock_ec2client(DESCRIBE_INSTANCES_BODY);
	let instances = get_instances_by_filters(&ec2, vec![]).unwrap();
        let actual = get_info_from_instances(instances).remove(0);
	let expected = make_expected_instance_info();
	assert_eq!(actual, expected);
    }


    #[test]
    fn test_parse_name_from_url() {
	let url = "https://v102rc2.demo.encodedcc.org".to_string();
	let name = parse_name_from_url(url).unwrap();
	assert_eq!(name, "v102rc2".to_owned());
	let url = "https://sno-157-b5bebd084-phil.demo.encodedcc.org/".to_string();
	let name = parse_name_from_url(url).unwrap();
	assert_eq!(name, "sno-157-b5bebd084-phil".to_owned());
	let url = "https://encd-5358-d5b93454a-emma.demo.encodedcc.org/".to_string();
	let name = parse_name_from_url(url).unwrap();
	assert_eq!(name, "encd-5358-d5b93454a-emma".to_owned());
    }


    #[test]
    fn test_is_instance_id() {
	let id = "i-02e86c27e5d31f8d1".to_string();
	assert_eq!(true, is_instance_id(id));
	let id = "i-02e86c27e5d311".to_string();
	assert_eq!(true, is_instance_id(id));
	let id = "encd-5358-d5b93454a-emma".to_string();
	assert_eq!(false, is_instance_id(id));
	let id = "http:i-02e86c27e5d31f8d1".to_string();
	assert_eq!(false, is_instance_id(id));
    }


    #[test]
    fn test_get_instance_ids_from_url_or_id() {
	let ec2 = make_mock_ec2client(DESCRIBE_INSTANCES_BODY);
        let url = "https://encd-5358-d5b93454a-emma.demo.encodedcc.org/".to_string();
	let instance_ids = get_instance_ids_from_url_or_id(&ec2, url);
	assert_eq!(instance_ids, vec!["i-0c3cbd3a6e1b8ffc8".to_owned()]);
	let id = "i-0c3cbd3a6e1b8ffc7".to_string();
	let instance_ids = get_instance_ids_from_url_or_id(&ec2, id);
	assert_eq!(instance_ids, vec!["i-0c3cbd3a6e1b8ffc7".to_owned()]);
    }
}
