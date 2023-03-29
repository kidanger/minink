drop table logs;

create virtual table logsfts using fts5(
    service,
    message
);

create table logs (
    hostname text not null,
    timestamp timestamp not null,
    logsfts_id integer
);

create index idx_logs_timestamp on logs(timestamp);
create index idx_logs_hostname on logs(hostname);
create index idx_logs_logsfts_id on logs(logsfts_id);
