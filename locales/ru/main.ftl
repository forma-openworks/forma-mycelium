# Самоназвание языка — показывается в настройках.
language-name = Русский

## Меню

menu-file = Файл
menu-view = Вид
menu-window = Окно
menu-help = Справка
menu-recent-projects = Недавние проекты
menu-recent-empty = Пока пусто
menu-open-editor = Открыть редактор
menu-new-workspace = Новое рабочее пространство

## Действия

action-open-project = Открыть проект мода…
action-close-project = Закрыть проект
action-settings = Настройки…
action-quit = Выход
action-about = О программе
action-return-windows = Вернуть все отдельные окна
action-reset-workspace = Сбросить раскладку пространства
action-reset-all-workspaces = Сбросить все пространства
action-done = Готово
action-close = Закрыть
action-rename = Переименовать
action-cancel = Отмена

dialog-open-project = Папка проекта мода

## Рабочие пространства

workspace-code = Код
workspace-map = Карта
workspace-assets = Ассеты
workspace-debug = Отладка
workspace-untitled = Пространство
workspace-copy-name = { $name } (копия)
workspace-new-copy = Копия текущего
workspace-rename = Переименовать…
workspace-rename-title = Название пространства
workspace-duplicate = Дублировать
workspace-delete = Удалить

## Строка состояния

status-no-project = Проект не открыт
status-renderer = Рендер
status-detached-windows = { $count ->
    [one] { $count } отдельное окно
    [few] { $count } отдельных окна
   *[many] { $count } отдельных окон
}

## Экран приветствия

welcome-tagline = Среда разработки модов для Minecraft: Fabric и NeoForge, Java и Kotlin. Понимает Майнкрафт, а не просто Java.
welcome-continue = Продолжить без проекта
welcome-recent = Недавние проекты

## Настройки

settings-title = Настройки
settings-language = Язык интерфейса
settings-language-system = Как в системе
settings-theme = Тема
settings-accent = Акцентный цвет
settings-font-size = Размер текста
settings-locales-hint = Свои переводы можно положить в { $path } (папка языка с файлами .ftl) — они подхватятся при следующем запуске.
theme-dark = Тёмная
theme-light = Светлая
view-zoom-hint = Масштаб: Ctrl+= / Ctrl+- / Ctrl+0

accent-violet = Фиолетовый
accent-blue = Синий
accent-teal = Бирюзовый
accent-green = Зелёный
accent-amber = Янтарный
accent-rose = Розовый

## О программе

about-version = Версия { $version }
about-description = Среда разработки модов для Minecraft, которая подсвечивает и объясняет всё в контексте Майнкрафта: исключения, миксины, ресурсы и связи внутри мода.
about-forma = Часть проекта FORMA

## Сообщения

toast-project-missing = Папка проекта не найдена: { $path }
error-vulkan = Не удалось запустить Forma Mycelium через Vulkan.

    Проверьте, что драйвер видеокарты поддерживает Vulkan.

    Подробности: { $details }

## Проект и раскладка

action-save-all = Сохранить всё
menu-run = Запуск
layout-save-default = Сохранить раскладку как стандартную
layout-save-default-hint = Эту раскладку получат все проекты, у которых ещё нет своей
layout-apply-default = Применить стандартную раскладку
layout-reset-default = Сбросить стандартную раскладку к заводской
status-problems = Открыть окно исключений
toast-project-initialized = Проект инициализирован: создана папка .mycelium
toast-default-layout-saved = Раскладка сохранена как стандартная
toast-default-layout-failed = Не удалось сохранить стандартную раскладку
toast-default-layout-applied = Применена стандартная раскладка
toast-default-layout-reset = Стандартная раскладка сброшена к заводской
toast-file-missing = Файл не найден: { $path }
toast-source-missing = Исходник не найден: { $class }
toast-no-sources = Нет исходников для { $class } ({ $jar })
toast-sync-failed = Не удалось получить модель проекта от Gradle: { $error }
status-syncing = Синхронизация с Gradle
toast-definition-missing = Определение не найдено
status-lsp-off = подсказки выключены
status-lsp-error = ошибка jdtls
status-lsp-busy = jdtls разбирает проект
status-lsp-hint = Подсказки Java (jdtls): ошибки и предупреждения во всём проекте
