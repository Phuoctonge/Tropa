"""Create a clean local portable test folder and ZIP without user data."""

import argparse
import hashlib
import json
import shutil
import zipfile
from pathlib import Path

from tropa_bundle import INVENTORY, PINNED as COMPONENT_PINS, inventory_bytes, verify_archive
from tropa_build import verify_build
from tropa_engine import runtime_pin, verify_engine_build


ROOT = Path(__file__).resolve().parent.parent
PINNED = {Path(name).name: checksum for name, checksum in COMPONENT_PINS.items()}


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/clash-verge.exe")
    parser.add_argument("--name", default="Tropa-portable-test")
    parser.add_argument("--build-record", type=Path, required=True)
    parser.add_argument("--source-snapshot", type=Path, required=True)
    parser.add_argument("--engine-build", type=Path, required=True)
    args = parser.parse_args()
    if not args.name or any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_" for character in args.name):
        parser.error("Name must contain only ASCII letters, digits, hyphens or underscores")
    binary = args.binary.resolve(strict=True)
    go = ROOT / ".tools/go1.26.3/go/bin/go.exe"
    engine = verify_engine_build(ROOT, args.engine_build, go)
    engine_pin = runtime_pin(engine)
    provenance = verify_build(ROOT, binary, args.source_snapshot, args.build_record)
    if provenance.get("engine") != engine_pin:
        raise ValueError("Packaged worker does not match recorded compilation")
    resources = ROOT / "src-tauri/resources"
    for name, expected in PINNED.items():
        if digest(resources / name) != expected:
            raise ValueError(f"Unexpected checksum for {name}")
    output_root = ROOT / ".artifacts/portable"
    output_root.mkdir(parents=True, exist_ok=True)
    output = output_root / args.name
    archive = output.with_suffix(".zip")
    if output.exists() or archive.exists():
        raise FileExistsError("Output already exists; choose another --name")
    output.mkdir()
    (output / "resources").mkdir()
    (output / "licenses").mkdir()
    shutil.copyfile(binary, output / "Tropa.exe")
    for name in PINNED:
        shutil.copyfile(resources / name, output / "resources" / name)
    shutil.copyfile(args.engine_build / "tropa-probe.exe", output / "resources/tropa-probe.exe")
    if digest(output / "resources/tropa-probe.exe") != engine_pin["sha256"]:
        raise ValueError("Worker binary does not match verified engine build")
    shutil.copyfile(ROOT / "LICENSE", output / "licenses/GPL-3.0.txt")
    shutil.copyfile(resources / "licenses/WarpScout.txt", output / "licenses/WarpScout.txt")
    for notice in INVENTORY["notices"]:
        shutil.copyfile(ROOT / notice["sourceFile"], output / notice["file"])
    shutil.copyfile(ROOT / "THIRD_PARTY_NOTICES.md", output / "THIRD_PARTY_NOTICES.md")
    (output / "component-inventory.json").write_bytes(inventory_bytes(engine_pin))
    (output / "README.txt").write_text(
        "Tropa — тестовая portable-сборка для Windows x64\n\n"
        "Для поставляемого ядра нужен процессор уровня x86-64-v2; WebView2 Runtime требуется отдельно.\n"
        "Распакуйте всю папку и запустите Tropa.exe. Установщик не нужен.\n"
        "При запуске подтвердите запрос администратора Windows.\n"
        "Затем нажмите переключатель подключения.\n"
        "Автоматический режим подберёт рабочее подключение. Сервер можно выбрать вручную.\n"
        "Кнопка «Сервер» расширяет основное окно и открывает таблицу подключений.\n"
        "«Найти серверы» выполняет расширенную проверку (до 10 минут); поиск можно остановить.\n"
        "После поиска остаются счётчики проверок; непроверенные варианты показаны отдельно от отказов.\n"
        "Пинг адреса, пинг туннеля и потери берутся из собственного поиска; UDP DNS — в подробностях.\n"
        "Выбор другого сервера при активном соединении отключает его и запускает новое подключение.\n"
        "Режим «Весь компьютер» направляет трафик компьютера через WARP.\n"
        "В «Раздельном туннелировании» можно выбрать EXE или папки с ними; остальные приложения идут напрямую.\n"
        "Добавление EXE включает выборочный режим. Для Firefox Portable определяется внутренний firefox.exe.\n"
        "Перед изменением режима или списка приложений отключитесь. Launcher и его дочерние процессы могут требовать отдельных EXE.\n"
        "По умолчанию крестик скрывает окно в трей; в настройках можно выбрать полный выход.\n"
        "Пункт «Выход» в трее отключает туннель и закрывает приложение.\n\n"
        "Настройки и регистрация создаются в папке data рядом с Tropa.exe.\n"
        "Не передавайте другим людям свою папку data: она содержит ключи устройства.\n"
        "В этом архиве регистраций и персональных настроек нет.\n\n"
        "Ограничения: локальная тестовая сборка, не готовый публичный выпуск.\n"
        "В коде доступны WG/AWG и MASQUE HTTP/3/HTTP/2; новые режимы ещё требуют нативных проверок.\n"
        "Раздельная маршрутизация UDP/IPv6 и совместимость с античитами ещё не подтверждены.\n"
        "Измерения для конкретной игры пока не доступны.\n"
        "Требуется имеющийся WebView2 Runtime Windows. Запуск на чистой Windows ещё не проверен.\n\n"
        "Состав и лицензии: THIRD_PARTY_NOTICES.md и component-inventory.json.\n"
        "Проверка транзитивных лицензий и соответствующих исходников ещё не завершена.\n\n"
        "Основа: Clash Verge Rev — https://github.com/clash-verge-rev/clash-verge-rev (GPL-3.0).\n"
        "Ядро: mihomo — https://github.com/MetaCubeX/mihomo (GPL-3.0).\n"
        "Поиск: собственный probe-движок; протокольные алгоритмы адаптированы из WarpScout (MIT).\n",
        encoding="utf-8-sig",
    )
    files = sorted(path for path in output.rglob("*") if path.is_file())
    manifest = {str(path.relative_to(output)).replace("\\", "/"): digest(path) for path in files}
    build_info = {"build": "development-test", "files": manifest}
    verify_build(ROOT, binary, args.source_snapshot, args.build_record)
    if manifest["Tropa.exe"] != provenance["binary"]["sha256"]:
        raise ValueError("Packaged binary differs from recorded compilation")
    build_info["localBuild"] = provenance
    (output / "build-info.json").write_text(json.dumps(build_info, indent=2), encoding="utf-8")
    with zipfile.ZipFile(archive, "x", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as package:
        for path in sorted(output.rglob("*")):
            if path.is_file():
                package.write(path, Path(args.name) / path.relative_to(output))
    verify_archive(archive, engine_build=args.engine_build, build_record=args.build_record,
                   source_snapshot=args.source_snapshot, root=ROOT)
    print(json.dumps({"folder": str(output), "archive": str(archive), "bytes": archive.stat().st_size}))


if __name__ == "__main__":
    main()
