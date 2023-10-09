create table users
(
    id         varchar(36) not null
        primary key,
    username   varchar(32) not null,
    email      varchar(128)
        unique,
    profile_id varchar(36)
);

alter table users
    owner to voxelphile;

INSERT INTO xenotech.users (id, username, email, profile_id) VALUES ('83ae6c57-4778-4f44-8b35-e1677741279e', 'brynn', 'brynnbrancamp@gmail.com', '00864fcd-5c85-4ad4-9f58-ba06f644b789');
