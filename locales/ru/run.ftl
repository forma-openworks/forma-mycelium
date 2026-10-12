# Запуск, консоль и окно исключений.

run-client = Клиент
run-server = Сервер
run-data = Генерация данных
run-build = Сборка
run-other = Задача
run-clean = Очистка
run-no-configs = Нет конфигураций запуска
run-start = Запустить
run-stop = Остановить
run-restart = Перезапустить
run-seconds = { $count } с
run-status-running = { $name } · идёт { $time }
run-status-done = { $name } · готово за { $time }
run-status-failed = { $name } · ошибка (код { $code }) через { $time }
run-status-stopped = { $name } · остановлено через { $time }
run-status-error = Не удалось запустить: { $error }

console-no-project-title = Нет проекта
console-no-project-hint = Откройте проект мода, чтобы запускать клиент, сервер и сборку.
console-empty-title = Пока тихо
console-empty-hint = Запустите клиент кнопкой ▶ (Shift+F10) — здесь появится лог, разобранный по системам игры и происхождению.
console-filtered-title = Ничего не подходит
console-filtered-hint = Под выбранные фильтры не попала ни одна запись.
console-clear = Очистить консоль
console-autoscroll = Прокручивать к новым записям
console-only-own = Только мой мод
console-only-problems = Только проблемы
console-level-all = Все уровни
console-level-info = INFO и серьёзнее
console-level-warn = Предупреждения и ошибки
console-level-error = Только ошибки
console-search = Поиск
console-filters = Фильтры
console-show-in-exceptions = Показать в окне исключений

exceptions-by-system = Система
exceptions-by-type = Тип
exceptions-by-origin = Происхождение
exceptions-by-severity = Серьёзность
exceptions-only-errors = Только ошибки
exceptions-clear = Очистить список
exceptions-empty-title = Проблем нет
exceptions-empty-hint = Запустите клиент — сюда попадут все исключения, ошибки и предупреждения, сгруппированные по системам игры.
exceptions-pick = Выберите проблему, чтобы увидеть подробности и объяснение.
exceptions-new = новое
exceptions-new-in-run = впервые в этом запуске
exceptions-not-in-last-run = в последнем запуске не повторилось
exceptions-count = { $count ->
    [one] { $count } раз
    [few] { $count } раза
   *[many] { $count } раз
}
exceptions-severity-crash = Краши
exceptions-severity-error = Ошибки
exceptions-severity-warning = Предупреждения
exceptions-type-error = Ошибки в логе
exceptions-type-warning = Предупреждения в логе
exceptions-noise-group = Безобидный шум
exceptions-noise-hint = Предупреждения, которые игра, загрузчик и другие моды пишут при каждом запуске в среде разработки. К вашему моду они отношения не имеют.
exceptions-only-noise = Настоящих проблем нет — только безобидный шум.

noise-tag = шум
noise-title = Безобидный шум
noise-explain = Это предупреждение Minecraft, загрузчик или другой мод пишут при каждом запуске в среде разработки, с любым модом. Исправлять в своём коде нечего — его можно спокойно пропустить.

frame-open-library-source = Открыть исходник (только чтение)
frame-open-source = Открыть исходник
frames-hidden = { $count ->
    [one] { $count } кадр
    [few] { $count } кадра
   *[many] { $count } кадров
}
frames-omitted = { $count ->
    [one] ещё { $count } кадр совпадает с исключением выше
    [few] ещё { $count } кадра совпадают с исключением выше
   *[many] ещё { $count } кадров совпадают с исключением выше
}

origin-own = Мой мод
origin-mod = Другой мод
origin-mod-named = Мод { $id }
origin-library = Библиотека
origin-unknown = Неизвестно

system-mod-loading = Загрузка модов
system-registries = Реестры
system-resources = Ресурсы
system-data = Данные
system-mixin = Миксины
system-network = Сеть
system-rendering = Отрисовка
system-sound = Звук
system-world = Мир
system-config = Конфиги
system-build = Сборка
system-game = Игра

run-edit-configs = Изменить конфигурации…
run-close-tab = Закрыть вкладку
run-stop-all = Остановить все
console-waiting-title = Запуск идёт
console-waiting-hint = Gradle готовится — лог появится, как только задача начнёт писать.

runcfg-title = Конфигурации запуска
runcfg-new = Новая
runcfg-new-name = Новая конфигурация
runcfg-duplicate = Сделать копию
runcfg-copy-name = { $name } (копия)
runcfg-hide = Скрыть из списка запуска
runcfg-show = Вернуть в список запуска
runcfg-delete = Удалить
runcfg-hidden = скрыта
runcfg-empty = Конфигураций нет — добавьте свою кнопкой «Новая».
runcfg-detected-note = Найдена в проекте. Правки сохраняются поверх найденной, её всегда можно вернуть как было.
runcfg-custom-note = Своя конфигурация.
runcfg-name = Название
runcfg-kind = Вид
runcfg-kind-hint = От вида зависят значок и статус в Discord.
runcfg-task = Задачи Gradle
runcfg-args = Аргументы Gradle
runcfg-reset = Вернуть как было
runcfg-saved-in = Хранится в .mycelium/runs.json — можно класть в git.

problem-via = через
problem-owner-hint = Чья это проблема: кому её исправлять
problem-via-hint = Кто её заметил или сообщил о ней

# Токены в сообщениях: подсказки и действия по клику.
token-class = Класс
token-target = Целевой класс
token-config = Конфиг mixin
token-mixin = Mixin
token-mod = Мод
token-resource = id ресурса
token-file = Файл
token-click = клик
token-target-hint = Класс игры, который mixin должен был изменить.
token-nested-hint = После $ идёт вложенный класс: { $inner } объявлен внутри { $outer }.
token-class-absent-hint = В этой версии такого класса нет.
token-class-moved-hint = В этой версии такого класса нет, зато есть { $class } — похоже, класс переехал.
token-config-hint = Список mixin-классов мода: какие mixin в нём есть и на какой стороне применяется каждый.
token-config-declared-hint = Подключён в { $file }.
token-config-package = Пакет: { $package }
token-mixin-hint = Сам mixin. Имя записано относительно пакета из конфига.
token-mixin-side-client-hint = Записан в секции «client»: применяется только в клиенте игры.
token-mixin-side-server-hint = Записан в секции «server»: применяется только на выделенном сервере.
token-mixin-side-mixins-hint = Записан в секции «mixins»: применяется и на клиенте, и на сервере.
token-mixin-later-hint = Переход к mixin чужого мода появится вместе с индексом байткода.
token-mod-hint = Мод, которому принадлежит mixin.
token-resource-hint = id ресурса: пространство имён (обычно id мода) и путь внутри assets/ или data/.
token-open-minecraft-source = Открыть исходник Minecraft
token-open-file = Открыть файл
token-open-from-jar = Открыть из jar мода
token-show-mod = Показать во вкладке «Моды запуска»

# Карточка проблемы.
problem-report = Сообщить автору
problem-report-hint = Копирует готовый отчёт (версии и строки лога) и открывает трекер задач мода.
problem-open-code = Открыть в коде
problem-open-code-hint = Открывает место в вашем моде, где это случилось.
problem-open-resource = Открыть { $name }
problem-open-resource-hint = Открывает файл вашего мода, о котором эта проблема.
problem-copy = Копировать
problem-copied = Скопировано
problem-copy-hint = Копирует строки лога как есть, вместе со стеком вызовов.
problem-stack-show = Показать стек вызовов
problem-stack-hide = Скрыть стек вызовов
problem-stack-yours = { $count } из вашего мода
problem-side-client = Клиент
problem-side-client-hint = Записал клиент игры.
problem-side-server = Сервер
problem-side-server-hint = Записал выделенный сервер.
problem-thread = Поток
thread-startup-hint = Поток запуска: загружает игру до появления окна. Здесь применяются mixin.
thread-render-hint = Логический клиент: отрисовка и ввод.
thread-server-hint = Логический сервер: игровая логика мира. В одиночной игре он работает внутри клиента.
thread-worker-hint = Фоновый поток: загрузка ресурсов и чанков.
thread-mod-loading-hint = Поток загрузки модов: конструкторы модов и события загрузки.
thread-network-hint = Сетевой поток.
thread-io-hint = Поток чтения и записи на диск.
