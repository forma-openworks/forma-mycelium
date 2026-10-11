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
exceptions-noise-group = Безобидный шум Minecraft
exceptions-noise-hint = Предупреждения, которые игра и загрузчик пишут при каждом запуске в среде разработки. К вашему моду они отношения не имеют.
exceptions-only-noise = Настоящих проблем нет — только безобидный шум.

noise-tag = шум
noise-title = Безобидный шум
noise-explain = Это предупреждение Minecraft или загрузчик пишут при каждом запуске в среде разработки, с любым модом. Исправлять в своём коде нечего — его можно спокойно пропустить.

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
