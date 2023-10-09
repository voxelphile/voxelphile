create table user_password_logins
(
    id            varchar(36) not null
        primary key
        constraint fk_id
            references users,
    password_hash varchar(128)
);

alter table user_password_logins
    owner to voxelphile;

INSERT INTO xenotech.user_password_logins (id, password_hash) VALUES ('83ae6c57-4778-4f44-8b35-e1677741279e', '$argon2id$v=19$m=19456,t=2,p=1$RfQ2qSunYpKmqzYgIRCJtA$86Wr/13zw+Pw6PsOjncUxymo1Fh9SaALGncAK8MUOJk');
