create table voxelphile.users
(
    id         varchar(36) not null
        primary key,
    username   varchar(32) not null,
    email      varchar(128)
        unique,
    profile_id varchar(36)
);

alter table voxelphile.users
    owner to voxelphile;

create table voxelphile.user_password_logins
(
    id            varchar(36) not null
        primary key
        constraint fk_id
            references voxelphile.users,
    password_hash varchar(128)
);

alter table voxelphile.user_password_logins
    owner to voxelphile;