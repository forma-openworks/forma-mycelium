# Редакторы во вкладках.

editor-project = Проект
editor-code = Код
editor-mod-map = Карта мода
editor-exceptions = Исключения
editor-console = Консоль
editor-model-preview = Превью модели

editor-project-about = Файлы проекта мода.
editor-code-about = Редактор Java и Kotlin с подсказками jdtls и kotlin-lsp, проверками в контексте Майнкрафта и миксинами, которые понятны с первого взгляда.
editor-mod-map-about = Граф классов и ресурсов мода: связи строятся сами, узлы можно собирать в группы, как фреймы в Blender, и подписывать стикерами. Кольцо вокруг узла показывает, от чего он зависит: Minecraft, NeoForge, Fabric или другие моды.
editor-exceptions-about = Все исключения и ошибки, сгруппированные по типам и системам игры, с историей между запусками и объяснениями простыми словами.
editor-console-about = Запуск клиента и сервера и лог, разобранный по серьёзности, системе и происхождению.
editor-model-preview-about = 3D-превью моделей блоков и предметов: JSON-модели Майнкрафта и OBJ, состояния блоков, режимы отображения и текстуры.
editor-coming-stage = Появится на этапе { $stage }

project-none-title = Проект не открыт
project-none-hint = Откройте папку с Gradle-проектом мода — Fabric или NeoForge.
project-refresh = Перечитать файлы

code-empty-title = Файл не открыт
code-empty-hint = Откройте файл двойным щелчком в дереве проекта или щёлкните по строке своего мода в стек-трейсе.
code-cannot-open = Файл не открывается как текст
code-binary = Это двоичный файл.
code-not-utf8 = Файл не в кодировке UTF-8.
code-too-large = Файл слишком большой для редактора.

project-collapse-all = Свернуть всё
project-show-hidden = Показать скрытые файлы и папки
project-hide-hidden = Спрятать скрытые файлы и папки
project-not-a-mod = Загрузчик не найден — это не похоже на мод
project-no-gradle = Нет build.gradle — это не Gradle-проект
project-open = Открыть
project-reveal = Показать в проводнике
project-copy-path = Копировать путь
project-copy-relative = Копировать путь от корня проекта
code-readonly = только чтение

libs-title = Внешние библиотеки
libs-syncing = синхронизация с Gradle…
libs-indexing = разбор библиотек…
libs-sync-failed = Gradle не ответил
libs-sync-failed-hint = Проверьте build.gradle и сеть, затем нажмите «Синхронизировать с Gradle» в меню этой строки. Подробный вывод — в журнале программы.
libs-sync = Синхронизировать с Gradle
libs-mods = Другие моды
libs-libraries = Библиотеки
libs-mojang = Библиотеки Mojang
libs-loader-parts = Части загрузчика
libs-has-sources = Исходники есть — классы открываются как код
libs-no-sources = Исходников нет
libs-copy-class = Копировать имя класса
libs-reveal-jar = Показать jar в проводнике

code-completion-loading = Ищу варианты…
lsp-not-installed = Подсказки Java выключены: нужен jdtls (около 50 МБ).
lsp-install = Установить
lsp-downloading-of = Скачиваю jdtls: { $done } из { $total } МБ
lsp-downloading = Скачиваю jdtls: { $done } МБ
lsp-extracting = Распаковываю jdtls…
lsp-no-jdk = Для подсказок Java нужен JDK { $version } или новее — он не найден.
lsp-starting = Запускаю jdtls…
lsp-importing = jdtls разбирает проект
lsp-failed = jdtls не работает: { $error }
lsp-restart = Перезапустить
