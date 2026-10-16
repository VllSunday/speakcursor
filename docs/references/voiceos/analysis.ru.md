> Исследование VoiceOS от 15 октября 2026 года. Это справочный материал, а не спецификация SpeakCursor. Актуальные решения пользователя: [состав продукта](../../product-scope.md) и [передача контекста](../../handoff.md).

**VoiceOS: возможности и варианты развития SpeakCursor**

Исследование публичных материалов на 15 октября 2026 года. В доступном журнале обновлений последним указан выпуск **0.1.37 от 5 октября 2026 года**; страница содержит 27 выпусков. Изучены основные страницы продукта, каталог всех 21 опубликованных интеграций, журнал обновлений, документация расширений, руководства по сценариям, цены и политика данных.

Это анализ опубликованных возможностей и текущего кода SpeakCursor. Десктопный VoiceOS не устанавливался, вход в чужие сервисы и реальные отправки сообщений не выполнялись. «Документировано» здесь означает наличие конкретного первичного источника, а не собственный успешный тест. Оценки трудоёмкости ниже — относительная оценка по архитектуре, без обещания сроков.

**Что представляет собой продукт**

VoiceOS — настольное приложение WakoAI Inc. для macOS и Windows. Название «voice operating system» описывает голосовой слой поверх обычных приложений. Пользователь вызывает его горячей клавишей, говорит и получает текст, правку, ответ или результат действия. Ввод в текстовое поле и отдельная интеграция с API приложения — разные возможности.

Три центральных режима:

| Режим | Что подаёт пользователь | Что получает |
| --- | --- | --- |
| Dictation | Новую речь | Распознанный и при необходимости отредактированный текст в поле ввода |
| Edit | Выделенный текст и голосовое указание | Новую версию выделенного фрагмента на его месте |
| Agent | Вопрос или поручение, при необходимости экран, файлы или подключённые сервисы | Ответ, черновик, карточку действия либо результат выполнения |

Основания: [обзор функций](https://www.voiceos.com/features), [диктовка](https://www.voiceos.com/features/dictation), [руководство по редактированию](https://www.voiceos.com/blog/rewrite-edit-text-by-voice-mac-windows), [Agent Mode](https://www.voiceos.com/features/agent).

**1. Диктовка и обработка текста**

- Системный голосовой ввод в стандартные поля: почта, мессенджеры, документы, заметки, задачи, поля запросов к ИИ, редакторы кода.
- Автоматическая пунктуация, регистр, абзацы и форматирование списков.
- Удаление слов-паразитов, запинок и некоторых неудачных начал фразы.
- Исправление распознанных слов и написания терминов с использованием контекста.
- Три степени обработки: **Raw, Light, Polished**. Это степень изменения текста, а не три разных пользовательских поручения агенту.
- Выбор обработки с учётом длины записи; в журнале описаны быстрый и более точный пути распознавания, резервные модели и повторная попытка при пустом результате.
- Поддержка порядка 100 языков и автоматического определения языка. В руководстве есть пример смешивания японского и английского; качество русского и русско-английской смеси нужно оценивать отдельно.
- Диктовка до 20 минут за сессию: предупреждение на 19-й минуте, автоматическое завершение и вставка на 20-й.
- Учёт места вставки: границы предложений, регистр, переносы, знаки препинания.
- В журнале много отдельных исправлений потери первых/последних слов, фокуса и вставки. Надёжность — самостоятельная часть продукта.

Защищённые поля и нестандартные редакторы могут работать иначе. Формулировка «в любом приложении» не равна гарантии совместимости каждого элемента интерфейса.

Источники: [Dictation](https://www.voiceos.com/features/dictation), [Changelog](https://www.voiceos.com/changelog), [Features](https://www.voiceos.com/features).

**2. Словарь, персонализация и стили**

- Пользовательский словарь для имён, компаний, названий продуктов, аббревиатур и профессиональных терминов.
- Пакетное добавление терминов через Spelling, включая помощь ИИ при импорте.
- Предварительный просмотр, редактирование и удаление предложенных терминов при первоначальной настройке.
- Обучение словаря на исправлениях, сделанных в Edit Mode — конкретно этот сценарий указан в обновлениях.
- Синхронизация словарных записей между устройствами упоминается в журнале.
- Replacements — пользовательские замены; их наличие отражено в ценах и политике данных. Полноценный язык шаблонов, переменные и универсальные текстовые макросы публичными материалами не установлены.
- Writing styles: пользовательские инструкции о стиле, сохранение предпочтений, более осторожная работа с техническим текстом.
- Адаптация к приложению: один и тот же замысел может стать оформленным письмом, коротким сообщением или списком заметок.
- Memories и knowledge entries: сохранённые предпочтения и сведения для персонализации. В журнале есть закрепляемые основные предпочтения.
- Для организаций описаны общие словари, замены, Memories и интеграции.

Не следует автоматически считать, что каждое из этих средств настраивается отдельно для любого сайта или поля: конкретный уровень детализации профилей публично не описан.

Источники: [Dictation](https://www.voiceos.com/features/dictation), [Changelog](https://www.voiceos.com/changelog), [Pricing](https://www.voiceos.com/pricing), [Privacy §§3,8](https://www.voiceos.com/privacy).

**3. Голосовое редактирование выделенного текста**

Основной цикл: **выделить → вызвать режим → произнести изменение → получить замену в том же поле**.

Документированные примеры:

- Сократить текст, в том числе примерно вдвое.
- Сделать формальнее, дружелюбнее, прямее или спокойнее.
- Исправить только грамматику и пунктуацию.
- Убрать повторы и слова-паразиты.
- Превратить абзац в пункты.
- Изменить структуру и сделать запрос яснее.
- Перевести текст на другой язык — описано в руководстве по голосовому вводу.

Редактирование текста само по себе не отправляет письмо и не публикует сообщение. В Agent Mode выделение может быть прикреплённым контекстом для вопроса, без замены содержимого.

Ссылка /features/edit, на которую ссылается один из руководств, на дату проверки вернула 404. Сценарий подтверждается общей страницей функций, подробным руководством и журналом обновлений.

Источники: [Edit workflow](https://www.voiceos.com/blog/rewrite-edit-text-by-voice-mac-windows), [Voice input guide](https://www.voiceos.com/blog/voice-input-complete-guide-2026), [Features](https://www.voiceos.com/features).

**4. Контекст экрана и указание мышью**

- Ответы о видимом тексте, письме, форме, ошибке, графике или интерфейсе.
- Снимок экрана по запросу, после включения функции и предоставления разрешения.
- Указание курсором, линия или обведение области во время вопроса: «объясни это», «как ответить на это сообщение».
- Слова, вопрос и изображение с отметкой передаются вместе.
- Обновление контекста для нового хода диалога, чтобы ответ не опирался на старый экран.
- Предварительный просмотр скриншотов.
- Вложения: изображения, файлы, вставка из буфера и перетаскивание.
- Сохранение выбранного текста и приложенных изображений в рамках последующих уточнений.
- Отдельный сценарий **Create Prompt**: проговорить задачу, отметить нужные части экрана и получить готовый запрос с пронумерованными скриншотами для другого ИИ.

Экранный контекст описан как захват по запросу, а не непрерывная запись дня. Есть расхождения по платформам: июльская статья про жест указывает Mac как проверенную платформу; более поздние материалы и обновления уже описывают работу с экраном и рисованием в Windows. Проверку конкретного жеста и нескольких мониторов на Windows нужно делать в текущей сборке.

Источники: [screen-aware guide](https://www.voiceos.com/blog/ai-that-can-see-your-screen-and-answer-questions), [Circle to Search](https://www.voiceos.com/blog/circle-to-search-on-your-computer), [Changelog](https://www.voiceos.com/changelog), [Privacy §5](https://www.voiceos.com/privacy).

**5. Агент и разговор**

- Вопросы и ответы без перехода в отдельный чат.
- Суммаризация видимого текста и подключённых документов.
- Подготовка ответа на письмо или сообщение.
- Поиск в интернете с источниками; в обновлениях есть изображения, карты и превью YouTube.
- Продолжение разговора с уточнениями, сохранение контекста при перебивании.
- Вывод ответов текстом и голосом.
- Настраиваемая скорость голоса, отключение озвучивания.
- В августовском обновлении перечислены голоса Sky, Dawn, Dusk, Midnight и Jarvis.
- Специализированные карточки результатов: почта, календарь, файлы, таблицы, графики, погода, медиа.
- Память о пользовательских предпочтениях, например единицах температуры.
- Открытие приложений и сайтов; в журнале есть выбор браузера/профиля и предпочтение установленного приложения перед сайтом.
- Цепочки действий между подключёнными сервисами, если нужные инструменты доступны.

Публичная документация не раскрывает полный состав и версии моделей. Упоминания Whisper для части облачных маршрутов не означают локальный Whisper у пользователя. Сведения о чужих голосовых продуктах в блоге также нельзя переносить на VoiceOS.

Источники: [Agent Mode](https://www.voiceos.com/features/agent), [Features](https://www.voiceos.com/features), [Changelog](https://www.voiceos.com/changelog).

**6. Действия, подтверждения, расписание и фоновые задачи**

- Работа с внешними сервисами после подключения учётной записи или выдачи локального разрешения.
- Редактируемые карточки перед частью действий: проверить адресата, текст, календарь, время или параметры.
- Возможность подтвердить, исправить или отменить предложение.
- Настройки подтверждения отдельных инструментов.
- Напоминания на естественном языке, запланированные действия и повторяющиеся проверки, например почты.
- Длительные поручения в фоне с состоянием, уведомлением о завершении и возможностью остановки.
- Боковая панель задач и возврат к результату из истории.
- Переключение задач: Option+Tab на Mac, Alt+обратная кавычка в Windows — по последнему журналу.
- Фоновое исследование интернета и отдельные задачи кодирующих агентов.

**Подтверждение не универсально.** На главных страницах встречается широкое обещание «ничего не отправляется без согласия», но конкретный каталог помечает действия индивидуально. В нём создание Apple Notes/Reminders, часть локальных действий и запуск задач Codex/Claude Code не имеют отметки Asks first. Действия Spotify исполняются сразу. Отсутствие отметки в каталоге не доказывает точное поведение любой версии приложения; оно означает, что отдельное подтверждение там не заявлено.

Источники: [App Store FAQ](https://www.voiceos.com/app-store), [40 source-checked actions](https://www.voiceos.com/blog/voice-agent-workflows), [Changelog](https://www.voiceos.com/changelog), [confirmation SDK](https://docs.voiceos.com/integrations/confirmations).

**7. Все опубликованные интеграции**

На дату проверки основной каталог содержит **21 интеграцию и 175 именованных инструментов**. Это счётчик инструментов каталога, не независимых полноценных сценариев: один инструмент может объединять чтение и поиск, другой — несколько операций с таблицей.

| Интеграция | Инструментов | Платформы по карточке | Основные возможности |
| --- | ---: | --- | --- |
| iMessage | 5 | Mac | Чтение, поиск, непрочитанные, отправка |
| Apple Mail | 7 | Mac | Чтение и поиск, отправка, ответы, перемещение |
| Reminders | 5 | Mac | Списки, поиск, создание, завершение |
| Finder | 12 | Mac | Поиск, чтение, открытие, создание, перемещение и удаление файлов |
| Apple Notes | 6 | Mac | Папки, поиск, чтение, создание, дополнение |
| Spotify | 9 | Mac в каталоге | Воспроизведение, громкость, перемотка, shuffle/repeat |
| Slack | 9 | Mac, Windows | Сообщения, поиск, каналы, реакции, расписание, файлы |
| Gmail | 7 | Mac, Windows | Письма, ответы, поиск, ярлыки, удаление, контакты |
| Google Calendar | 7 | Mac, Windows | Календари, свободное время, создание, изменение и удаление событий |
| Apple Calendar | 7 | Mac | Календари, поиск, свободное время, управление событиями |
| Google Maps | 1 | Mac в каталоге | Открытие места, адреса или маршрута |
| Outlook | 11 | Mac, Windows | Почта, календарь и контакты Microsoft 365 |
| Google Drive | 17 | Mac, Windows | Файлы, доступ, Google Docs и Sheets |
| Linear | 10 | Mac, Windows | Задачи, комментарии, приоритеты, метки, команды, циклы |
| Jira | 11 | Mac, Windows | Задачи, статусы, назначения, вложения, проекты, спринты |
| Claude Code | 8 | Mac, Windows | Кодирующие задачи, состояние, результаты и ответы агентам |
| Codex | 8 | Mac, Windows | Кодирующие задачи, состояние, результаты и ответы агентам |
| Notion | 9 | Mac, Windows | Страницы, блоки, базы, строки, комментарии, файлы |
| Obsidian | 4 | Mac, Windows | Поиск, чтение, создание и дополнение заметок |
| Canvas | 13 | Mac, Windows | Учебные курсы, задания, оценки, сдача работ, сообщения |
| X | 9 | Mac, Windows | Публикации, поиск, лента, профили и личные сообщения |

В каталоге beta отмечены Slack, Spotify, Claude Code, Codex, Obsidian и X.

Более поздние обновления дополнительно описывают **Apple Music, Grok Bot и Manus**, которых в просмотренном основном каталоге нет. Для Grok Bot прямо указан Mac. У X доступ к чтению и личным сообщениям зависит от тарифа API. У Spotify и карт возможности в журнале шире, чем в статичных карточках каталога. Поэтому каталог и changelog приведены раздельно, без обещания одинаковой доступности всех функций на обеих ОС.

Полный перечень всех 175 инструментов с исходными названиями и отметками подтверждений приведён в приложении в конце.

Источники: [официальный каталог](https://www.voiceos.com/app-store), отдельные страницы интеграций в приложении, [Changelog](https://www.voiceos.com/changelog).

**8. Повседневный интерфейс**

- Вызов удержанием клавиши.
- Режим без удержания: двойное нажатие запускает запись, следующее завершает.
- Настраиваемые клавиши; в обновлениях также есть кнопки мыши как триггер.
- Отмена Escape.
- Плавающая компактная панель записи, индикатор обработки и значок активного приложения.
- Верхняя область ответа — notch; боковая область задач и истории.
- Настройки показа плавающей панели, скрытия верхнего/бокового элемента, светлой, тёмной и системной темы.
- Приглушение другой музыки во время записи и озвучивания.
- Выбор микрофона и восстановление после его отключения или сна компьютера.
- Управление фокусом и восстановление буфера, чтобы результат попадал в исходное поле.
- История диктовок и диалогов, копирование текста с сохранением структуры.
- Поиск и возврат к прошлым задачам/ответам.
- Локальное сохранение записи для восстановления, если повторная транскрипция не удалась.
- Открытие длинного ответа в отдельном окне и сохранение в PDF.
- Insights: скорость речи, объём диктовки, оценка сэкономленного времени, перцентиль и карточка Voice Type.
- Обучение при первом запуске, практические уроки, разрешения, поддержка и обновления приложения.
- Запуск в фоне при входе в Windows.

Важное продуктовое наблюдение: большая часть выпускаемых исправлений относится к старту микрофона, фокусу, вставке, горячим клавишам, потере текста и восстановлению после ошибок. Копирование только внешнего вида не воспроизведёт ощущение надёжного продукта.

Источник: [Changelog](https://www.voiceos.com/changelog).

**9. Расширяемость и конструктор интеграций**

Это самостоятельный большой пласт возможностей, который можно целиком исключить из небольшой версии:

- Подключение собственных MCP-серверов: локальная команда или удалённый URL.
- MCP — стандартный способ описать доступные агенту инструменты и их параметры.
- Integration Studio: описать интеграцию словами, ответить на уточнения, получить код, карточки и пробный запуск.
- Доработка через диалог, тестирование реальных вызовов, настройка необходимых ключей и параметров.
- Полученный результат — папка с манифестом и сервером, которую можно открывать в редакторе.
- Установка и обмен интеграциями, ссылки для установки, ручная проверка публикации в каталоге.
- Карточки результата и редактируемые карточки подтверждения.
- Собственные HTML-виджеты внутри изолированной поверхности интерфейса.
- Права на сетевые домены, уведомления, фоновые действия и доступ к транскрипту.
- Hooks: обработка готовой диктовки перед вставкой, дополнение контекста, обработка собственных вызовов инструментов.
- Fast intents: короткие однозначные команды могут быстрее попадать в существующий инструмент; обычный агент остаётся резервным путём.
- Обработка длительных задач и уведомления о завершении.
- Хранение и передача пользовательских параметров/ключей отдельно от текста разговора.

SDK обозначен как **developer preview**. Облачный hosted runtime и некоторые элементы контракта помечены как зарезервированные, а не доступные. Документация местами отстаёт от changelog: например, ограничение количества параллельных сборок Studio различается. Обещанные «три минуты» и бюджет fast-intent selector не являются общим временем отклика приложения.

Источники: [SDK](https://docs.voiceos.com/integrations), [архитектура](https://docs.voiceos.com/integrations/how-it-works), [Studio](https://docs.voiceos.com/integrations/build-with-ai), [Hooks](https://docs.voiceos.com/integrations/hooks), [Fast intents](https://docs.voiceos.com/integrations/intents), [Limits](https://docs.voiceos.com/integrations/reference/limits).

**10. Платформы, стоимость и данные**

Платформы: macOS и Windows. Мобильная страница сейчас предлагает waitlist. Упоминание iOS-клавиатуры в старом changelog не доказывает открытый выпуск мобильного приложения. Поддержка Linux не заявлена.

Справочные цены в США:

| Предложение | Условия |
| --- | --- |
| Pro помесячно | $29.99 в месяц |
| Pro за год | $143.88 за год, эквивалент $11.99 в месяц |
| Пробный период | 7 дней, требуется карта, затем начинается выбранная подписка |
| Enterprise | Индивидуальные условия, SSO/SAML и администрирование уточняются у продавца |
| После окончания оплаченного Pro у существующего аккаунта | 100 диктовок и 25 Agent-сессий в неделю |
| Скидки | 50% студентам после проверки; создатели интеграций могут подать заявку |

Свободного бесплатного тарифа для новой регистрации по текущим ценам нет. В отдельных карточках интеграций и старых материалах сохранилось «Free to start». Региональные цены и условия оплаты могут отличаться.

VoiceOS использует **облачную обработку**. Локальная история не означает локальное распознавание.

При выключенном Help Improve VoiceOS политика обещает удаление сырого аудио и транскриптов сразу после необходимой обработки и отсутствие использования перечисленного содержимого для обучения/оценки моделей VoiceOS. При добровольном включении настройки аудио и транскрипты могут храниться **бессрочно для обучения**; выключение настройки и даже удаление аккаунта не удаляют ранее предоставленные под этой настройкой материалы. Учётная запись, персонализация, интеграционные токены и операционные сведения регулируются отдельно; для account-linked данных указан срок удаления до 40 дней.

Главная страница формулирует отсутствие хранения и обучения слишком широко. Для решения об архитектуре нужно опираться на политику, которая описывает условия и исключения.

Security описывает TLS, шифрование хранилищ, AWS и работу над SOC 2 Type I. Это не подтверждение уже полученной сертификации. Публичных сопоставимых измерений точности, задержки и реальной экономии времени в исследованных материалах нет; рекламные 4×, 10×, 98% и 300 ms не использованы как проверенные характеристики.

Источники: [Pricing](https://www.voiceos.com/pricing), [pricing.md](https://www.voiceos.com/pricing.md), [Mobile](https://www.voiceos.com/mobile), [Privacy](https://www.voiceos.com/privacy), [Security](https://www.voiceos.com/security).

**11. Что уже есть в SpeakCursor**

По коду текущего checkout:

- Приложение для Windows в системном трее.
- Удержание F8 для диктовки; Shift+F8 для структурирования новой диктовки.
- Локальное faster-whisper, GPU при наличии и CPU fallback.
- Выбор размера Whisper-модели и языка.
- Необязательная обработка через OpenAI с собственным ключом.
- Обычный промпт исправления текста и smart-промпт для структуры Markdown.
- Вставка в активное окно; отдельная комбинация для терминалов; режим имитации ввода.
- Сохранение/восстановление текстового содержимого буфера.
- Последние 20 текстовых результатов.
- Windows autostart и выгрузка модели в простое.

Текущий smart-режим обрабатывает новую речь. Отдельного получения выделенного текста, исполнения голосовой инструкции над ним, контекста экрана и вызова инструментов нет. Существующие промпты прямо задают считать надиктованные просьбы материалом для редактуры, а не поручениями.

Определение активного приложения уже есть для выбора Ctrl+V / Ctrl+Shift+V, но пока не используется для тона или содержания результата.

Код: [main.py](../../../main.py#L80), [gpt_formatter.py](../../../gpt_formatter.py#L3), [hotkeys.py](../../../hotkeys.py#L8), [clipboard.py](../../../clipboard.py#L33), [history.py](../../../history.py#L8), [settings.py](../../../settings.py#L13).

**12. Карта возможных доработок**

Ниже варианты для обсуждения, не согласованный план реализации.

| Возможность | Текущее состояние | Относительный объём |
| --- | --- | --- |
| Улучшенная очистка диктовки | Уже есть основа | Небольшой–средний |
| Raw / Light / Polished | GPT включён/выключен и два промпта | Небольшой |
| Пользовательский словарь и замены | Нет | Небольшой–средний |
| Выбираемые стили / пользовательские инструкции | Два встроенных сценария | Небольшой–средний |
| Двойное нажатие и запись без удержания | Нет | Небольшой–средний |
| Плавающая панель с состоянием и отменой | Состояние только в трее | Средний |
| История с поиском и восстановлением | Последние 20 текстов | Небольшой–средний |
| Редактирование выделенного текста | Нет | Средний |
| Перевод выделенного фрагмента | Нет; может стать операцией Edit | Средний вместе с Edit |
| Форматирование под активное приложение | Есть определение процесса только для вставки | Средний |
| Снимок экрана и вопрос к нему | Нет | Средний–большой |
| Create Prompt с отметками и скриншотами | Есть структурирование только текста | Средний–большой |
| Отдельный разговор с голосовым ответом | Нет | Средний–большой |
| Память о предпочтениях | Нет | Средний |
| Несколько конкретных интеграций | Нет | Большой, зависит от сервиса |
| Напоминания и фоновые поручения | Нет | Большой |
| Универсальная система MCP | Нет | Большой |
| Studio, магазин, SDK, общие организационные настройки | Нет | Очень большой |

«Небольшой» не означает одну строчку: совместимость горячих клавиш, фокус и буфер всё равно надо проверять в Windows. Редактирование зависит от способности надёжно получить выделение и вернуть результат; большой промпт сам этого не добавит.

Возможные направления:

- **Удобная диктовка:** словарь, степени очистки, стили, панель, запись без удержания, восстановление.
- **Редактор для работы с текстом:** всё нужное из диктовки плюс Edit, перевод, изменение тона и структуры.
- **Помощник по экрану:** вопросы к снимку, выделению и вложениям, подготовка запросов для других ИИ.
- **Агент под конкретные задачи:** только выбранные сервисы и операции с ними, без собственного магазина и конструктора.

Эти направления можно сочетать. Локальное распознавание SpeakCursor можно сохранить независимо от того, будут ли редактура и анализ изображений облачными.

**13. Что нужно решить до выбора первой версии**

1. Какие реальные действия должны стать быстрее: надиктовать, переписать выделение, объяснить экран, подготовить запрос в Cursor/Codex, отправить что-то через сервис?
2. Где приложение должно работать: только Windows или другие ОС тоже?
3. Какие языки действительно нужны: русский, английский, смесь терминов?
4. Нужно ли распознавание без интернета и можно ли отправлять текст/снимки на выбранный API?
5. Это личный инструмент с собственным ключом или приложение для других пользователей?
6. Нужны ли действия с внешним результатом — отправка, изменение календаря, создание задачи — или достаточно готового текста?
7. Какие приложения стоят в центре работы? Перечня из 3–5 обычно достаточно для первого набора проверок.
8. Насколько важны плавающий интерфейс, история, отмена и восстановление по сравнению с новыми ИИ-возможностями?

Требуется отдельная проверка выбранных сценариев на русском и в реальных Windows-приложениях. Точные сроки стоит оценивать после этого выбора.

Публичными источниками не установлены: постоянное включение по wake word, универсальное управление произвольными кнопками всех приложений, точность русского языка на пользовательском микрофоне, полная работа офлайн, возможность выбрать собственную локальную модель VoiceOS, гарантированное исполнение расписаний при выключенном или спящем компьютере и полный перечень внутренних моделей. Эти свойства нельзя включать в список подтверждённых только по рекламным роликам.

**Приложение: полный каталог 175 инструментов**

Названия и описания ниже сохранены на языке официального каталога, чтобы их можно было однозначно сопоставить с источником. «Есть отметка» означает Asks first в конкретной карточке. «Не указано» не равно обещанию отсутствия подтверждения во всех версиях.


**Apple Calendar** — macOS; 7 инструментов. [Источник](https://www.voiceos.com/app-store/apple-calendar).

| Действие | Описание | Подтверждение в каталоге |
| --- | --- | --- |
| Your calendars | See every calendar in the Calendar app. | Не указано |
| See your schedule | Read what's on for a day or a range of days. | Не указано |
| Search events | Find an event by what it's called. | Не указано |
| Find open time | Look for gaps you could book something into. | Не указано |
| Create an event | Add an event with a time, a place, and invitees. | Не указано |
| Update an event | Change the time, title, or details of one you have. | Не указано |
| Delete an event | Take an event off your calendar. | Не указано |

**Apple Mail** — macOS; 7 инструментов. [Источник](https://www.voiceos.com/app-store/apple-mail).

| Действие | Описание | Подтверждение в каталоге |
| --- | --- | --- |
| Unread email | List what's unread across every account in Mail, newest first. | Не указано |
| Read an email | Open one message in full, body and recipients included. | Не указано |
| Search mail | Find messages by subject, sender, mailbox, or date. | Не указано |
| Search inside emails | Look through message bodies when the subject line won't find it. | Не указано |
| Send an email | Compose and send a new message from your account. | Есть отметка |
| Reply to an email | Answer a thread in place, keeping its recipients. | Есть отметка |
| Move to another mailbox | File a message into a different mailbox. | Есть отметка |

**Notes** — macOS; 6 инструментов. [Источник](https://www.voiceos.com/app-store/apple-notes).

| Действие | Описание | Подтверждение в каталоге |
| --- | --- | --- |
| Your folders | See the folders you have in Notes. | Не указано |
| See notes | List the notes in a folder, newest first. | Не указано |
| Search notes | Find a note by its title or what's written in it. | Не указано |
| Read a note | Open one note in full. | Не указано |
| Create a note | Start a new note in the folder you name. | Не указано |
| Add to a note | Append text to the end of a note you already have. | Не указано |

**Canvas** — macOS · Windows; 13 инструментов. [Источник](https://www.voiceos.com/app-store/canvas).

| Действие | Описание | Подтверждение в каталоге |
| --- | --- | --- |
| Your courses | See what you're enrolled in this term. | Не указано |
| Assignments | Read what's been set, and when it's due. | Не указано |
| Grades and submissions | Check what you've handed in and what came back. | Не указано |
| Submit an assignment | Hand work in, or save it as a draft first. | Есть отметка |
| What's due next | See your to-do list and what's coming up. | Не указано |
| Course calendar | Read the events on your course calendars. | Не указано |
| Add a calendar event | Put something on your Canvas calendar. | Есть отметка |
| Course material | Work through modules and see recent course activity. | Не указано |
| Announcements and discussions | Catch up on what instructors and classmates posted. | Не указано |
| Post to a discussion | Reply in a course discussion thread. | Есть отметка |
| Your Canvas inbox | Read the messages sitting in Canvas. | Не указано |
| Send a message | Message an instructor or a classmate. | Есть отметка |
| Your profile | Confirm which Canvas account is connected. | Не указано |

**Claude Code** — macOS · Windows; 8 инструментов. [Источник](https://www.voiceos.com/app-store/claude-code).

| Действие | Описание | Подтверждение в каталоге |
| --- | --- | --- |
| Start a coding task | Hand a task to Claude Code in one of your project folders. | Не указано |
| See running tasks | Check which sessions are going and how far along they are. | Не указано |
| Read a task result | Pull up what a finished session changed and said. | Не указано |
| Answer a session | Reply by voice when the agent stops to ask you something mid-run. | Не указано |
| Run several agents at once | Split one job across parallel sessions in the same repo. | Не указано |
| See parallel runs | Check the status of every agent in a parallel run. | Не указано |
| Read a parallel run | Pull up the combined result once the agents finish. | Не указано |
| Answer a parallel run | Reply to whichever agent in the run is waiting on you. | Не указано |

**Codex** — macOS · Windows; 8 инструментов. [Источник](https://www.voiceos.com/app-store/codex).

| Действие | Описание | Подтверждение в каталоге |
| --- | --- | --- |
| Start a coding task | Hand a task to Codex in one of your project folders. | Не указано |
| See running tasks | Check which sessions are going and how far along they are. | Не указано |
| Read a task result | Pull up what a finished session changed and said. | Не указано |
| Answer a session | Reply by voice when the agent stops to ask you something mid-run. | Не указано |
| Run several agents at once | Split one job across parallel sessions in the same repo. | Не указано |
| See parallel runs | Check the status of every agent in a parallel run. | Не указано |
| Read a parallel run | Pull up the combined result once the agents finish. | Не указано |
| Answer a parallel run | Reply to whichever agent in the run is waiting on you. | Не указано |

**Finder** — macOS; 12 инструментов. [Источник](https://www.voiceos.com/app-store/finder).

| Действие | Описание | Подтверждение в каталоге |
| --- | --- | --- |
| Search for files | Find files by name, kind, or date anywhere on this Mac. | Не указано |
| Browse a folder | List what's inside a folder. | Не указано |
| Read a file | Read a document's contents into an answer. | Не указано |
| Get file info | Check size, kind, and dates for a file or folder. | Не указано |
| Recent files | See what you've worked on lately. | Не указано |
| Open a file | Open a file or folder in the app it belongs to. | Есть отметка |
| Create a folder | Make a new folder where you ask for it. | Есть отметка |
| Create a file | Write a new text file to disk. | Есть отметка |
| Save an attached file | Put a file you attached in the notch somewhere on disk. | Есть отметка |
| Move or rename | Move a file to another folder, or give it a new name. | Есть отметка |
| Copy | Duplicate a file or folder somewhere else. | Есть отметка |
| Move to Trash | Send it to the Trash — recoverable until you empty it. | Есть отметка |

**Gmail** — macOS · Windows; 7 инструментов. [Источник](https://www.voiceos.com/app-store/gmail).

| Действие | Описание | Подтверждение в каталоге |
| --- | --- | --- |
| Send an email | Compose and send a new message from your Gmail account. | Есть отметка |
| Reply to a thread | Answer an existing conversation in place. | Есть отметка |
| Read and search mail | Find messages and read them in full, by search or thread. | Не указано |
| Organize with labels | Label a message, and see which labels you already have. | Есть отметка |
| Move to trash | Trash a message or a whole thread, or pull one back out. | Есть отметка |
| Delete permanently | Erase a message or thread for good — no undo. | Есть отметка |
| Look up contacts | Find someone's address from the contacts on your account. | Не указано |

**Google Calendar** — macOS · Windows; 7 инструментов. [Источник](https://www.voiceos.com/app-store/google-calendar).

| Действие | Описание | Подтверждение в каталоге |
| --- | --- | --- |
| Create an event | Add a timed event with guests and a location. | Есть отметка |
| Create an all-day event | Add something that takes the whole day. | Есть отметка |
| See your schedule | Read what's on, and find one event in particular. | Не указано |
| Find open time | Look for gaps you could book something into. | Не указано |
| Update an event | Change the time, the title, or who's invited. | Есть отметка |
| Delete an event | Take an event off your calendar. | Есть отметка |
| Your calendars | See which calendars on the account it can work with. | Не указано |

**Google Drive** — macOS · Windows; 17 инструментов. [Источник](https://www.voiceos.com/app-store/google-drive).

| Действие | Описание | Подтверждение в каталоге |
| --- | --- | --- |
| Find files and folders | Search Drive and check what a file is. | Не указано |
| Create a file | Put a new text file into Drive. | Есть отметка |
| Create a folder | Make a new folder in Drive. | Есть отметка |
| Upload a file | Send a file from this computer up to Drive. | Есть отметка |
| Download a file | Pull a file down from Drive. | Не указано |
| Edit a file | Change the contents of a file already in Drive. | Есть отметка |
| Move or copy a file | Put it in another folder, or duplicate it. | Есть отметка |
| Share a file | Give someone access to a file or folder. | Есть отметка |
| Delete a file or folder | Remove it from Drive. | Есть отметка |
| Create a spreadsheet | Start a new Google Sheet. | Есть отметка |
| Read a spreadsheet | Find sheets, read cells, and look up rows. | Не указано |
| Update cells and rows | Write values, add rows, or clear a range. | Есть отметка |
| Add or remove sheets | Change the tabs, rows, and columns of a spreadsheet. | Есть отметка |
| Format cells | Change how cells look in a spreadsheet. | Не указано |
| Create a document | Start a new Google Doc from text you dictate. | Есть отметка |
| Read a document | Find a doc and read what's in it. | Не указано |
| Edit a document | Insert text, replace it, or rewrite the whole doc. | Есть отметка |

**Google Maps** — macOS; 1 инструментов. [Источник](https://www.voiceos.com/app-store/google-maps).

| Действие | Описание | Подтверждение в каталоге |
| --- | --- | --- |
| Open in Maps | Open a place, an address, or a route in Google Maps. | Не указано |

**iMessage** — macOS; 5 инструментов. [Источник](https://www.voiceos.com/app-store/imessage).

| Действие | Описание | Подтверждение в каталоге |
| --- | --- | --- |
| Recent conversations | See who you've been messaging and the latest message in each. | Не указано |
| Read a conversation | Pull up the recent messages in one thread. | Не указано |
| Search messages | Find messages by what they say, across your history. | Не указано |
| Unread messages | Check what came in while you were away. | Не указано |
| Send a message | Write and send an iMessage to a contact or a group. | Не указано |

**Jira** — macOS · Windows; 11 инструментов. [Источник](https://www.voiceos.com/app-store/jira).

| Действие | Описание | Подтверждение в каталоге |
| --- | --- | --- |
| Create an issue | File a new issue in a project, with the right fields. | Есть отметка |
| Edit an issue | Change its summary, description, or fields. | Есть отметка |
| Delete an issue | Remove an issue from Jira — this can't be undone. | Есть отметка |
| Find issues | Search with JQL and read an issue in full. | Не указано |
| Comment on an issue | Read the discussion, and add to it. | Есть отметка |
| Move an issue's status | Push it to In Progress, Done, or wherever it goes next. | Есть отметка |
| Assign an issue | Hand it to someone on the project. | Есть отметка |
| Attach a file | Add an attachment to an existing issue. | Есть отметка |
| Projects | See the projects in this site, and start a new one. | Есть отметка |
| Find people | Resolve a name to the account you want to assign. | Не указано |
| Boards and sprints | See the boards and sprints, and move an issue into one. | Есть отметка |

**Linear** — macOS · Windows; 10 инструментов. [Источник](https://www.voiceos.com/app-store/linear).

| Действие | Описание | Подтверждение в каталоге |
| --- | --- | --- |
| Create an issue | File a new issue on a team, with a title and details. | Есть отметка |
| Update an issue | Change its state, assignee, priority, or description. | Есть отметка |
| Delete an issue | Move an issue to Linear's trash. | Есть отметка |
| Find issues | Search your team's issues and read one in full. | Не указано |
| Comment on an issue | Add a comment to the thread. | Есть отметка |
| Manage labels | Create a label, or take one off an issue. | Есть отметка |
| Attach a file | Add an attachment to an existing issue. | Есть отметка |
| Teams and people | Look up a team or a teammate to assign work to. | Не указано |
| Projects and states | See the projects and workflow states you can use. | Не указано |
| Cycles and sprints | Check which cycle a team is in. | Не указано |

**Notion** — macOS · Windows; 9 инструментов. [Источник](https://www.voiceos.com/app-store/notion).

| Действие | Описание | Подтверждение в каталоге |
| --- | --- | --- |
| Create a page | Start a new page under a parent you name. | Есть отметка |
| Add content to a page | Append blocks of text to a page you already have. | Есть отметка |
| Update a page | Change a page's title or its properties. | Есть отметка |
| Search and read | Find a page in your workspace and read what's on it. | Не указано |
| Query a database | Filter and read rows out of a Notion database. | Не указано |
| Add a database row | Insert a new row with its properties filled in. | Есть отметка |
| Comment on a page | Leave a comment where the team will see it. | Есть отметка |
| Upload a file | Put a file into a page. | Есть отметка |
| Find people | Resolve a name before setting a person property. | Не указано |

**Obsidian** — macOS · Windows; 4 инструментов. [Источник](https://www.voiceos.com/app-store/obsidian).

| Действие | Описание | Подтверждение в каталоге |
| --- | --- | --- |
| Search your vault | Find notes by what's written in them. | Не указано |
| Read a note | Open one note from the vault. | Не указано |
| Create a note | Add a new note to the vault. | Не указано |
| Add to a note | Append text to a note you already have. | Не указано |

**Outlook** — macOS · Windows; 11 инструментов. [Источник](https://www.voiceos.com/app-store/outlook).

| Действие | Описание | Подтверждение в каталоге |
| --- | --- | --- |
| Send an email | Compose and send a new message from your account. | Есть отметка |
| Reply to an email | Answer a message without leaving the thread. | Есть отметка |
| Draft an email | Write a new draft, or a draft reply, for you to review. | Есть отметка |
| Read and search mail | List, search, and open messages in your mailbox. | Не указано |
| Move an email | File it into another folder — deleting means Deleted Items. | Есть отметка |
| Create a calendar event | Add an event with a time and attendees. | Есть отметка |
| Update a calendar event | Change the time, the title, or the attendees. | Есть отметка |
| Delete a calendar event | Take an event off the calendar. | Есть отметка |
| See your schedule | Read your calendar and check when you're free. | Есть отметка |
| Create a contact | Save someone new to your contacts. | Есть отметка |
| Look up contacts | Find someone's details in your contacts. | Не указано |

**Reminders** — macOS; 5 инструментов. [Источник](https://www.voiceos.com/app-store/reminders).

| Действие | Описание | Подтверждение в каталоге |
| --- | --- | --- |
| Your lists | See every Reminders list on this Mac. | Не указано |
| See reminders | Read what's due, what's coming up, and what's done. | Не указано |
| Search reminders | Find a reminder by what it says. | Не указано |
| Create a reminder | Add one with a due date and the list it belongs in. | Не указано |
| Complete a reminder | Tick one off once it's done. | Не указано |

**Slack** — macOS · Windows; 9 инструментов. [Источник](https://www.voiceos.com/app-store/slack).

| Действие | Описание | Подтверждение в каталоге |
| --- | --- | --- |
| Send a message | Post to a channel or DM someone directly. | Есть отметка |
| Schedule a message | Queue a message to go out at a later time. | Есть отметка |
| Read conversations | Catch up on a channel or a thread you've missed. | Не указано |
| Search messages | Find what someone said, across the workspace. | Не указано |
| Find channels and people | Look up a channel or a teammate by name or email. | Не указано |
| Open a DM | Start a direct message with someone. | Не указано |
| React to a message | Add an emoji reaction to something. | Есть отметка |
| Set a Slack reminder | Have Slack nudge you about something later. | Есть отметка |
| Upload a file | Share a file into a channel or a DM. | Есть отметка |

**Spotify** — macOS; 9 инструментов. [Источник](https://www.voiceos.com/app-store/spotify).

| Действие | Описание | Подтверждение в каталоге |
| --- | --- | --- |
| What's playing | Check the current track, its artist, and how far in. | Не указано |
| Play something | Start a song, artist, album, or playlist. | Не указано |
| Pause | Pause whatever is playing. | Не указано |
| Skip forward | Jump to the next track. | Не указано |
| Skip back | Go back to the previous track. | Не указано |
| Set the volume | Turn Spotify up or down. | Не указано |
| Shuffle | Turn shuffle on or off. | Не указано |
| Repeat | Repeat one track, the whole queue, or nothing. | Не указано |
| Seek | Jump to a point in the current track. | Не указано |

**X (Twitter)** — macOS · Windows; 9 инструментов. [Источник](https://www.voiceos.com/app-store/x).

| Действие | Описание | Подтверждение в каталоге |
| --- | --- | --- |
| Post | Publish a new post from your account. | Есть отметка |
| Like a post | Like someone else's post. | Не указано |
| Repost | Share someone else's post to your followers. | Есть отметка |
| Your timeline | Read posts from the accounts you follow. | Не указано |
| Search posts | Find recent posts, including your own mentions. | Не указано |
| Followers and following | See who follows an account and who it follows. | Не указано |
| Send a DM | Message someone directly. | Есть отметка |
| Read DMs | Catch up on your recent direct messages. | Не указано |
| Look up a profile | Resolve a username or account id to the person behind it. | Не указано |
