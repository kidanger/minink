create table logs (
    message text not null,
    hostname text not null,
    service text not null,
    timestamp timestamp not null
);

create index idx_logs_timestamp on logs(timestamp);
