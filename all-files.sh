#!/bin/bash
# Настройки: измените значения переменных по необходимости

# Базовая папка для обхода
# Базовая папка - корень проекта
BASE_DIR="."

# Разрешённые расширения файлов (включаем Rust-код, конфигурации и markdown-документы)
EXTENSIONS=("rs" "toml")

# Файлы и папки, которые нужно игнорировать (сборки, зависимости, скрытые файлы)
IGNORE=("target" ".git" "node_modules" "Cargo.lock" "system" "third_party" "tmp" "tika-service" ".cargo" "cfg" "cfg.bak2" "sources" ".fleet")

# Дополнительные файлы, которые всегда нужно включить
ADD_FILES=("$BASE_DIR/Dockerfile" "$BASE_DIR/docker-compose.yml" "$BASE_DIR/.env-example" "$BASE_DIR/README.md")
# Файл результата
OUTPUT_FILE="all-files.txt"

# Очищаем файл результата, если он существует
> "$OUTPUT_FILE"

# Функция для проверки наличия элемента в массиве
contains() {
    local element="$1"
    shift
    for item in "$@"; do
        if [[ "$item" == "$element" ]]; then
            return 0
        fi
    done
    return 1
}

# Функция проверки, содержит ли путь игнорируемый шаблон
should_ignore() {
    local path="$1"
    for pattern in "${IGNORE[@]}"; do
        if [[ "$path" == *"$pattern"* ]]; then
            return 0
        fi
    done
    return 1
}

write_file_content() {
    local file="$1"
    echo "$file"
    echo "$file" >> "$OUTPUT_FILE"
    echo '```' >> "$OUTPUT_FILE"
    cat "$file" >> "$OUTPUT_FILE"
    echo '```' >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"  # добавляем пустую строку для разделения
}


# Рекурсивный обход папки
while IFS= read -r -d '' file; do
    # Обрабатываем только обычные файлы
    if [ -f "$file" ]; then
        # Пропускаем, если файл или его путь содержит один из игнорируемых шаблонов
        if should_ignore "$file"; then
            continue
        fi

        # Определяем расширение файла
        ext="${file##*.}"
        # Если расширения нет или оно не входит в разрешённый список — пропускаем файл
        if ! contains "$ext" "${EXTENSIONS[@]}"; then
            continue
        fi

        # Вызываем функцию для записи содержимого файла
        write_file_content "$file"
    fi
done < <(find "$BASE_DIR" -type f -print0)

# Обработка дополнительных файлов
for f in "${ADD_FILES[@]}"; do
    if [ -f "$f" ]; then
        write_file_content "$f"
    fi
done

echo "Готово! Результат записан в $OUTPUT_FILE"
