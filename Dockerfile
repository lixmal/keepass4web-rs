FROM docker.io/rust:1-alpine as build

WORKDIR /workspace

COPY js js
COPY public public
COPY package*.json ./

RUN apk add --no-cache npm
RUN npm install
RUN cp node_modules/bootstrap/fonts/* public/fonts/
RUN npm run build

COPY src src
COPY Cargo.* ./

RUN apk add --no-cache build-base libressl libressl-dev
ENV RUSTFLAGS="-Ctarget-cpu=sandybridge -Ctarget-feature=+aes,+sse2,+sse4.1,+ssse3"
RUN cargo build --bins --release


FROM scratch

COPY --from=build /workspace/public /
COPY --from=build /workspace/target/release/keepass4web-rs /keepass4web
COPY config.yml /conf/

EXPOSE 8080

VOLUME /conf

USER 1000:1000

ENV RUST_BACKTRACE=1;

CMD [ "/keepass4web", "--config", "/conf/config.yml"]
