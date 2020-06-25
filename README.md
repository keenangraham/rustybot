# rustybot
Enbot in Rust

# Help
```
USAGE:
    <@rustybot> [COMMAND]
COMMANDS:
    *help*     Print this message
    *list*     List active jobs
    *stop/cancel [JOB_ID]*     Cancel job
    *monitor [URL]*     Monitor URL until indexing is complete
    *status [URL]*     Return URL indexer status and results
    *ec2 info [URL/ID]*    Get instance info
    *ec2 start [URL/ID]*    Start instance
    *ec2 stop [URL/ID]*    Stop instance
    *ec2 resize [URL/ID] -s/--size [SIZE]*     Resize instance (default r5.2xlarge)
    *ec2 ls -f/--filter [KEY=VALUE] -l/--limit [NUM]*     List instances with optional filters
EXAMPLES:
@rustybot list
@rustybot stop 1234
@rustybot status https://www.encodeproject.org/
@rustybot monitor https://test.encodedcc.org/
@rustybot ec2 info https://dev-84b292185-keenan.demo.encodedcc.org/
@rustybot ec2 info i-02e86c27e5d31f8d1
@rustybot ec2 start i-02e86c27e5d31f8d1
@rustybot ec2 stop https://dev-84b292185-keenan.demo.encodedcc.org/
@rustybot ec2 resize https://dev-84b292185-keenan.demo.encodedcc.org/
@rustybot ec2 resize i-02e86c27e5d31f8d1 --size c5.9xlarge
@rustybot ec2 ls --filter instance-type=t2.micro --limit 5
@rustybot ec2 ls -f instance-type=t2.micro -f instance-state-name=running -l 3
@rustybot ec2 ls -f tag:Name=dev-84b292185-keenan -f tag:started_by=keenan
```
