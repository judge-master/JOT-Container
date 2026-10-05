# 프로토콜 정의 파일
이 디렉터리에는 JOT의 프로토콜을 정의하는 파일을 만듭니다. 

프로토콜의 명세가 수정되면 직접 protoc를 실행해 src/generated/ 아래의 파일을 수동으로 생성해야 합니다.

## 생성 방법
먼저 시스템에 protoc가 설치되어 있어야 합니다. 운영체제에 따라 apt, pacman, winget 등의 패키지 매니저로 설치할 수 있습니다.

그 후 아래 명령을 실행해 protoc-gen-prost, protoc-gen-tonic 을 설치합니다.
```sh
cargo install protoc-gen-prost protoc-gen-tonic
```

이제 아래 명령을 실행하면 src/generated 아래에 파일이 생성됩니다.
```sh
protoc -I proto proto/judge/v1/judge.proto \
  --prost_out=src/generated \
  --tonic_out=src/generated
```
