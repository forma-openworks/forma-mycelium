# Самоназвание языка — показывается в настройках.
language-name = Русский

## Меню

menu-file = Файл
menu-view = Вид
menu-window = Окно
menu-help = Справка
window-minimize = Свернуть
window-maximize = Развернуть
window-restore = Свернуть в окно
window-close = Закрыть
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
menu-compact = Меню
workspace-more = { $count ->
    [one] Ещё { $count } рабочее пространство
    [few] Ещё { $count } рабочих пространства
   *[many] Ещё { $count } рабочих пространств
}

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

settings-discord = Показывать в Discord, над каким модом я работаю
settings-discord-secret = Секретный проект: скрыть название этого мода
settings-discord-secret-hint = Вместо названия — «секретный проект», без иконки мода и ссылки на его репозиторий.

settings-page-discord = Discord
settings-discord-secret-no-project = Откройте проект, чтобы скрыть его название.
settings-component-appearance = Язык, тема, акцентный цвет, размер текста, масштаб
settings-component-keymap = Ваши изменения сочетаний клавиш
settings-component-discord = Активность в Discord

menu-manage-settings = Управление настройками
action-export-settings = Экспорт настроек…
action-import-settings = Импорт настроек…
action-find-action = Найти действие…
action-sync-gradle = Синхронизировать с Gradle
action-run = Запустить выбранную конфигурацию
action-zoom-in = Увеличить масштаб
action-zoom-out = Уменьшить масштаб
action-zoom-reset = Сбросить масштаб
action-show-project = Окно «Проект»
action-show-console = Окно «Консоль»
action-show-exceptions = Окно «Исключения»
action-completion = Автодополнение
action-goto-definition = Перейти к определению

action-group-file = Файл
action-group-project = Проект
action-group-run = Запуск
action-group-view = Вид
action-group-window = Окно
action-group-editor = Редактор кода
action-group-help = Справка
settings-component-fonts = Шрифты интерфейса, редактора и консоли
settings-component-plugins = Какие плагины установлены и какие выключены
plugin-jdtls-name = Языковой сервер Java (jdtls)
plugin-jdtls-about = Ошибки прямо в коде, автодополнение, описания при наведении и переход к определению для Java — и в исходниках Minecraft.
plugin-kotlin-name = Языковой сервер Kotlin
plugin-kotlin-about = То же самое для модов на Kotlin.
plugin-python-name = Python
plugin-python-about = Скрипты и дополнения для Forma Mycelium, как в Blender.
plugin-vs-dark-name = Visual Studio Dark
plugin-vs-dark-about = Тёмная тема по мотивам Visual Studio 2022: синяя строка состояния и привычные цвета кода.
dialog-install-plugin = Установить плагин из файла
toast-plugin-installed = Плагин установлен: { $name }
toast-plugin-failed = Ошибка плагина: { $error }
toast-plugin-install-failed = Не удалось установить { $name }: { $error }
theme-new-name = Моя тема
toast-theme-created = Тема создана: { $path }
toast-theme-create-failed = Не удалось создать тему: { $error }
action-quick-theme = Быстрая смена темы…
