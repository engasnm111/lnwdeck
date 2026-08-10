# lnwdeck — End-User Guide / คู่มือผู้ใช้ / 用户指南 / ユーザーガイド

> This guide is available in English, ไทย (Thai), 简体中文 (Simplified
> Chinese), 日本語 (Japanese), 한국어 (Korean), Deutsch (German), Français
> (French), Español (Spanish) and Русский (Russian). Pick your language below.
>
> คู่มือนี้มีหลายภาษา — เลือกหัวข้อภาษาที่คุณอ่านง่ายที่สุดด้านล่าง
>
> 本指南提供多种语言版本，请选择下方您最熟悉的一种。
>
> このガイドは複数の言語に対応しています。一番読みやすい言語を選んでください。

---


---

## English

lnwdeck is a local-only Windows tracker that shows how many tokens and
requests each AI tool has used, and how much quota your provider still has.
All data stays on your machine; there is no account, server or cloud sync.

### How quota is collected

lnwdeck reads provider-reported quota. Each provider needs to be logged in on
**this machine** so lnwdeck can reuse the session you already granted:

| Provider | What to do on this machine |
|---|---|
| Gemini / Google / Antigravity | Log in to **Antigravity IDE** or the **Gemini CLI** once. lnwdeck reads the token from Windows Credential Manager automatically. |
| Claude | Run `claude` and log in once. |
| OpenAI Codex | Run `codex login` once. |
| Cursor | Log in to Cursor once. |
| Kimi Code | Log in to Kimi once. |
| OpenCode Go | Set the workspace id and auth cookie in **Settings → Provider credentials → OpenCode Go**. |

### Gemini / Google / Antigravity detailed quota

The Antigravity IDE shows two quota groups on its **Settings → Models** screen:
*Gemini Models* and *Claude and GPT models*, each with a *weekly* and a
*five-hour* limit. lnwdeck shows the same numbers, but it can only read them
while the **Antigravity IDE is running on this machine**, because Google only
issues that data to the IDE's own language server.

- **Antigravity IDE open** → lnwdeck shows the real weekly / 5-hour
  percentages, identical to the IDE.
- **Antigravity IDE closed** → lnwdeck falls back to the basic request-quota
  endpoint, which may show 100% even when you have used some quota. Open the
  IDE and press **Refresh all providers** to see the detailed numbers again.

The login token lives in Windows Credential Manager (`gemini:antigravity`).
lnwdeck never asks you to paste it, and it never leaves your machine.

### Troubleshooting

- **"Not connected"** — the provider is not installed or not logged in on
  this machine. Install / log in, then refresh.
- **"Authentication expired"** — re-login to the provider (Antigravity IDE,
  `claude`, `codex login`, Kimi, ...) and refresh again.
- **Gemini shows 100%** — the Antigravity IDE is not running, so lnwdeck
  cannot read the detailed weekly / 5-hour quota. Open the IDE and refresh.

---

## ไทย

lnwdeck เป็นโปรแกรมติดตามการใช้ AI บน Windows แบบทำงานในเครื่องเท่านั้น ใช้ดูว่า
แต่ละเครื่องมือ AI ใช้โทเคนไปเท่าไร เหลือโควต้าเท่าไร ข้อมูลทั้งหมดอยู่ในเครื่อง
ของคุณ ไม่มีบัญชี เซิร์ฟเวอร์ หรือการซิงก์ผ่านคลาวด์

### วิธีดึงโควต้า

lnwdeck อ่านโควต้าที่ผู้ให้บริการประกาศ ผู้ให้บริการแต่ละรายต้องล็อกอินบน
**เครื่องนี้** เพื่อให้ lnwdeck ใช้เซสชันที่คุณให้สิทธิ์ไว้แล้ว:

| ผู้ให้บริการ | สิ่งที่ต้องทำบนเครื่องนี้ |
|---|---|
| Gemini / Google / Antigravity | ล็อกอิน **Antigravity IDE** หรือ **Gemini CLI** ครั้งเดียว lnwdeck อ่านโทเคนจาก Windows Credential Manager ให้อัตโนมัติ |
| Claude | รัน `claude` แล้วล็อกอินครั้งเดียว |
| OpenAI Codex | รัน `codex login` ครั้งเดียว |
| Cursor | ล็อกอิน Cursor ครั้งเดียว |
| Kimi Code | ล็อกอิน Kimi ครั้งเดียว |
| OpenCode Go | ตั้งค่า Workspace ID และ auth cookie ใน **ตั้งค่า → ข้อมูลรับรองผู้ให้บริการ → OpenCode Go** |

### โควต้าแบบละเอียดของ Gemini / Google / Antigravity

Antigravity IDE แสดงโควต้า 2 กลุ่มบนหน้า **ตั้งค่า → โมเดล** ได้แก่ *Gemini
Models* และ *Claude and GPT models* โดยแต่ละกลุ่มมีขีดจำกัด *รายสัปดาห์* และ
*ห้าชั่วโมง* lnwdeck แสดงตัวเลขเดียวกันได้ แต่จะอ่านได้เฉพาะตอนที่
**Antigravity IDE เปิดอยู่บนเครื่องนี้** เพราะ Google ออกข้อมูลนี้ให้เฉพาะ
language server ของ IDE เท่านั้น

- **เปิด Antigravity IDE อยู่** → lnwdeck แสดงเปอร์เซ็นต์รายสัปดาห์ / 5 ชั่วโมง
  จริง ตรงกับใน IDE ทุกอย่าง
- **ปิด Antigravity IDE** → lnwdeck ใช้ endpoint โควต้าคำขอแบบพื้นฐาน ซึ่งอาจ
  แสดง 100% ทั้งที่คุณใช้โควต้าไปแล้ว เปิด IDE แล้วกด **รีเฟรชผู้ให้บริการ
  ทั้งหมด** เพื่อดูตัวเลขแบบละเอียดอีกครั้ง

โทเคนล็อกอินอยู่ใน Windows Credential Manager (`gemini:antigravity`) lnwdeck
ไม่เคยขอให้คุณวางโทเคน และไม่เคยส่งออกจากเครื่องของคุณ

### การแก้ปัญหา

- **ไม่มีการเชื่อมต่อ** — ผู้ให้บริการยังไม่ได้ติดตั้งหรือยังไม่ได้ล็อกอินบน
  เครื่องนี้ ติดตั้ง/ล็อกอิน แล้วรีเฟรชใหม่
- **การยืนยันตัวตนหมดอายุ** — ล็อกอินผู้ให้บริการใหม่ (Antigravity IDE,
  `claude`, `codex login`, Kimi, …) แล้วรีเฟรชใหม่
- **Gemini แสดง 100%** — Antigravity IDE ไม่ได้เปิดอยู่ lnwdeck จึงอ่านโควต้า
  รายสัปดาห์ / 5 ชั่วโมงแบบละเอียดไม่ได้ เปิด IDE แล้วรีเฟรช

---

## 简体中文

lnwdeck 是一款仅在本地运行的 Windows 用量追踪工具，用于查看每个 AI 工具使用了
多少令牌（token）以及提供商还剩多少配额。所有数据都保存在您的电脑上，没有
账户、服务器或云端同步。

### 如何获取配额

lnwdeck 读取提供商公布的配额。每个提供商都需要在**这台电脑上**登录，
这样 lnwdeck 才能复用您已授权的会话：

| 提供商 | 需要在这台电脑上做什么 |
|---|---|
| Gemini / Google / Antigravity | 登录一次 **Antigravity IDE** 或 **Gemini CLI**。lnwdeck 会自动从 Windows 凭据管理器中读取令牌。 |
| Claude | 运行 `claude` 并登录一次。 |
| OpenAI Codex | 运行 `codex login` 一次。 |
| Cursor | 登录一次 Cursor。 |
| Kimi Code | 登录一次 Kimi。 |
| OpenCode Go | 在 **设置 → 提供商凭据 → OpenCode Go** 中设置 Workspace ID 和身份验证 Cookie。 |

### Gemini / Google / Antigravity 详细配额

Antigravity IDE 在 **设置 → 模型** 页面显示两组配额：*Gemini Models* 和
*Claude and GPT models*，每组都有*每周*和*五小时*限制。lnwdeck 可以显示相同
的数字，但只能在 **Antigravity IDE 在这台电脑上运行** 时读取，因为 Google
只向 IDE 自己的语言服务器提供这些数据。

- **Antigravity IDE 已打开** → lnwdeck 显示真实的每周 / 5 小时百分比，与
  IDE 完全一致。
- **Antigravity IDE 已关闭** → lnwdeck 回退到基本的请求配额接口，即使您已
  使用部分配额也可能显示 100%。打开 IDE 并点击**全部刷新**即可再次查看
  详细数字。

登录令牌位于 Windows 凭据管理器（`gemini:antigravity`）。lnwdeck 从不要求
您粘贴令牌，也绝不会将其传出您的电脑。

### 故障排查

- **未连接** — 提供商未在这台电脑上安装或未登录。请安装/登录后刷新。
- **身份验证已过期** — 重新登录提供商（Antigravity IDE、`claude`、
  `codex login`、Kimi 等）后再次刷新。
- **Gemini 显示 100%** — Antigravity IDE 未运行，lnwdeck 无法读取详细的
  每周 / 5 小时配额。请打开 IDE 并刷新。

---

## 日本語

lnwdeck は Windows 向けのローカル専用トラッカーで、各 AI ツールが使用した
トークン数と、プロバイダーの残りクォータを表示します。すべてのデータは
お使いのマシンに留まり、アカウント・サーバー・クラウド同期はありません。

### クォータの取得方法

lnwdeck はプロバイダーが公開するクォータを読み取ります。各プロバイダーは
**このマシン**でログイン済みである必要があります（lnwdeck はあなたが既に
許可したセッションを再利用します）：

| プロバイダー | このマシンで行うこと |
|---|---|
| Gemini / Google / Antigravity | **Antigravity IDE** または **Gemini CLI** に一度ログインします。lnwdeck は Windows 資格情報マネージャーから自動的にトークンを読み取ります。 |
| Claude | `claude` を実行して一度ログインします。 |
| OpenAI Codex | `codex login` を一度実行します。 |
| Cursor | Cursor に一度ログインします。 |
| Kimi Code | Kimi に一度ログインします。 |
| OpenCode Go | **設定 → プロバイダー資格情報 → OpenCode Go** で Workspace ID と認証 Cookie を設定します。 |

### Gemini / Google / Antigravity の詳細クォータ

Antigravity IDE の **設定 → モデル** 画面には、*Gemini Models* と
*Claude and GPT models* の 2 グループがあり、それぞれ*毎週*と*5時間*の
制限があります。lnwdeck は同じ数値を表示できますが、**Antigravity IDE が
このマシンで起動している間**のみ読み取れます。Google はこのデータを IDE の
言語サーバーにのみ提供するためです。

- **Antigravity IDE が起動中** → lnwdeck は IDE と完全に同じ毎週 / 5時間の
  実パーセンテージを表示します。
- **Antigravity IDE が停止中** → lnwdeck は基本的なリクエスト・クォータの
  エンドポイントにフォールバックするため、一部使用済みでも 100% と表示
  されることがあります。IDE を開いて**すべて更新**を押すと詳細な数値が
  再表示されます。

ログイントークンは Windows 資格情報マネージャー（`gemini:antigravity`）に
保存されています。lnwdeck がトークンの貼り付けを求めることはなく、トークン
がマシンの外に出ることもありません。

### トラブルシューティング

- **接続されていません** — プロバイダーがこのマシンにインストール/ログイン
  されていません。インストール/ログインしてから更新してください。
- **認証の有効期限が切れました** — プロバイダー（Antigravity IDE、
  `claude`、`codex login`、Kimi など）に再ログインしてから更新してください。
- **Gemini が 100% と表示される** — Antigravity IDE が起動していないため、
  lnwdeck は詳細な毎週 / 5時間クォータを読み取れません。IDE を開いて更新
  してください。

---

## 한국어

lnwdeck는 Windows용 로컬 전용 트래커로, 각 AI 도구가 사용한 토큰 수와
프로바이더의 남은 할당량을 보여줍니다. 모든 데이터는 내 컴퓨터에만 남으며
계정, 서버, 클라우드 동기화가 없습니다.

### 할당량 가져오는 방법

lnwdeck는 프로바이더가 공개한 할당량을 읽습니다. 각 프로바이더는 **이
컴퓨터**에서 로그인되어 있어야 합니다(lnwdeck는 이미 승인된 세션을
재사용합니다):

| 프로바이더 | 이 컴퓨터에서 할 일 |
|---|---|
| Gemini / Google / Antigravity | **Antigravity IDE** 또는 **Gemini CLI**에 한 번 로그인합니다. lnwdeck가 Windows 자격 증명 관리자에서 토큰을 자동으로 읽습니다. |
| Claude | `claude`를 실행하고 한 번 로그인합니다. |
| OpenAI Codex | `codex login`을 한 번 실행합니다. |
| Cursor | Cursor에 한 번 로그인합니다. |
| Kimi Code | Kimi에 한 번 로그인합니다. |
| OpenCode Go | **설정 → 프로바이더 자격 증명 → OpenCode Go**에서 Workspace ID와 인증 쿠키를 설정합니다. |

### Gemini / Google / Antigravity 상세 할당량

Antigravity IDE의 **설정 → 모델** 화면에는 *Gemini Models*와 *Claude and
GPT models* 두 그룹이 있으며, 각각 *주간* 및 *5시간* 제한이 있습니다.
lnwdeck도 같은 숫자를 표시할 수 있지만, **Antigravity IDE가 이 컴퓨터에서
실행 중일 때만** 읽을 수 있습니다. Google이 이 데이터를 IDE의 언어 서버에만
제공하기 때문입니다.

- **Antigravity IDE가 열려 있음** → lnwdeck가 IDE와 동일한 실제 주간 / 5시간
  백분율을 표시합니다.
- **Antigravity IDE가 닫혀 있음** → lnwdeck가 기본 요청 할당량 엔드포인트로
  대체되어, 일부를 사용했어도 100%로 표시될 수 있습니다. IDE를 열고 **모두
  새로고침**을 누르면 상세 숫자를 다시 볼 수 있습니다.

로그인 토큰은 Windows 자격 증명 관리자(`gemini:antigravity`)에 있습니다.
lnwdeck는 토큰을 붙여넣으라고 요구하지 않으며, 토큰이 내 컴퓨터 밖으로
나가지 않습니다.

### 문제 해결

- **연결되지 않음** — 프로바이더가 이 컴퓨터에 설치/로그인되지 않았습니다.
  설치/로그인 후 새로고침하세요.
- **인증 만료** — 프로바이더(Antigravity IDE, `claude`, `codex login`,
  Kimi 등)에 다시 로그인하고 새로고침하세요.
- **Gemini가 100%로 표시됨** — Antigravity IDE가 실행되지 않아 lnwdeck가
  상세 주간 / 5시간 할당량을 읽을 수 없습니다. IDE를 열고 새로고침하세요.

---

## Deutsch

lnwdeck ist ein lokaler Tracker für Windows, der anzeigt, wie viele Token jedes
KI-Tool verbraucht hat und wie viel Kontingent Ihr Anbieter noch hat. Alle Daten
bleiben auf Ihrem Gerät; es gibt kein Konto, keinen Server und keine
Cloud-Synchronisierung.

### So wird das Kontingent ermittelt

lnwdeck liest das vom Anbieter gemeldete Kontingent. Jeder Anbieter muss auf
**diesem Gerät** angemeldet sein, damit lnwdeck die bereits gewährte Sitzung
wiederverwenden kann:

| Anbieter | Was Sie auf diesem Gerät tun müssen |
|---|---|
| Gemini / Google / Antigravity | Melden Sie sich einmal bei **Antigravity IDE** oder der **Gemini CLI** an. lnwdeck liest das Token automatisch aus dem Windows-Anmeldeinformations-Manager. |
| Claude | Führen Sie `claude` aus und melden Sie sich einmal an. |
| OpenAI Codex | Führen Sie `codex login` einmal aus. |
| Cursor | Melden Sie sich einmal bei Cursor an. |
| Kimi Code | Melden Sie sich einmal bei Kimi an. |
| OpenCode Go | Legen Sie Workspace-ID und Auth-Cookie unter **Einstellungen → Anbieter-Anmeldedaten → OpenCode Go** fest. |

### Detailliertes Kontingent für Gemini / Google / Antigravity

Die Antigravity IDE zeigt auf dem Bildschirm **Einstellungen → Modelle** zwei
Kontingentgruppen: *Gemini Models* und *Claude and GPT models*, jeweils mit
*wöchentlichem* und *Fünf-Stunden*-Limit. lnwdeck zeigt dieselben Zahlen, kann
sie aber nur lesen, solange die **Antigravity IDE auf diesem Gerät läuft**, weil
Google diese Daten nur dem eigenen Language Server der IDE ausstellt.

- **Antigravity IDE geöffnet** → lnwdeck zeigt die echten wöchentlichen /
  5-Stunden-Prozentsätze, identisch zur IDE.
- **Antigravity IDE geschlossen** → lnwdeck fällt auf den einfachen
  Anfragekontingent-Endpunkt zurück, der auch bei teilweise verbrauchtem
  Kontingent 100 % anzeigen kann. Öffnen Sie die IDE und drücken Sie **Alle
  aktualisieren**, um die detaillierten Zahlen wieder zu sehen.

Das Anmeldetoken liegt im Windows-Anmeldeinformations-Manager
(`gemini:antigravity`). lnwdeck fordert Sie nie auf, es einzufügen, und es
verlässt nie Ihr Gerät.

### Fehlerbehebung

- **Nicht verbunden** — Der Anbieter ist auf diesem Gerät nicht installiert
  oder nicht angemeldet. Installieren/anmelden und dann aktualisieren.
- **Anmeldung abgelaufen** — Melden Sie sich erneut beim Anbieter an
  (Antigravity IDE, `claude`, `codex login`, Kimi, ...) und aktualisieren Sie.
- **Gemini zeigt 100 %** — Die Antigravity IDE läuft nicht, daher kann lnwdeck
  das detaillierte wöchentliche / 5-Stunden-Kontingent nicht lesen. Öffnen Sie
  die IDE und aktualisieren Sie.

---

## Français

lnwdeck est un outil de suivi local pour Windows qui indique combien de
tokens chaque outil d'IA a utilisés et combien de quota il reste chez votre
fournisseur. Toutes les données restent sur votre machine : pas de compte,
pas de serveur, pas de synchronisation cloud.

### Comment le quota est collecté

lnwdeck lit le quota publié par le fournisseur. Chaque fournisseur doit être
connecté sur **cette machine** pour que lnwdeck puisse réutiliser la session
que vous avez déjà autorisée :

| Fournisseur | À faire sur cette machine |
|---|---|
| Gemini / Google / Antigravity | Connectez-vous une fois à **Antigravity IDE** ou à la **Gemini CLI**. lnwdeck lit le jeton automatiquement dans le Gestionnaire d'identifiants Windows. |
| Claude | Exécutez `claude` et connectez-vous une fois. |
| OpenAI Codex | Exécutez `codex login` une fois. |
| Cursor | Connectez-vous une fois à Cursor. |
| Kimi Code | Connectez-vous une fois à Kimi. |
| OpenCode Go | Définissez l'ID d'espace de travail et le cookie d'authentification dans **Paramètres → Identifiants du fournisseur → OpenCode Go**. |

### Quota détaillé Gemini / Google / Antigravity

L'écran **Paramètres → Modèles** d'Antigravity IDE affiche deux groupes de
quota : *Gemini Models* et *Claude and GPT models*, chacun avec une limite
*hebdomadaire* et une limite de *cinq heures*. lnwdeck affiche les mêmes
chiffres, mais ne peut les lire que lorsque **Antigravity IDE est en cours
d'exécution sur cette machine**, car Google ne transmet ces données qu'au
serveur de langage de l'IDE.

- **Antigravity IDE ouvert** → lnwdeck affiche les pourcentages hebdomadaires /
  5 heures réels, identiques à l'IDE.
- **Antigravity IDE fermé** → lnwdeck revient à l'endpoint de quota de
  requêtes de base, qui peut afficher 100 % même si vous avez déjà consommé du
  quota. Ouvrez l'IDE et appuyez sur **Tout actualiser** pour revoir les
  chiffres détaillés.

Le jeton de connexion se trouve dans le Gestionnaire d'identifiants Windows
(`gemini:antigravity`). lnwdeck ne vous demande jamais de le coller et il ne
quitte jamais votre machine.

### Dépannage

- **Non connecté** — Le fournisseur n'est pas installé ou pas connecté sur
  cette machine. Installez/connectez-vous, puis actualisez.
- **Authentification expirée** — Reconnectez-vous au fournisseur (Antigravity
  IDE, `claude`, `codex login`, Kimi, ...) puis actualisez.
- **Gemini affiche 100 %** — Antigravity IDE ne tourne pas, donc lnwdeck ne
  peut pas lire le quota hebdomadaire / 5 heures détaillé. Ouvrez l'IDE et
  actualisez.

---

## Español

lnwdeck es un rastreador local para Windows que muestra cuántos tokens ha
usado cada herramienta de IA y cuánta cuota le queda a su proveedor. Todos los
datos permanecen en su equipo; no hay cuenta, servidor ni sincronización en la
nube.

### Cómo se recopila la cuota

lnwdeck lee la cuota que publica el proveedor. Cada proveedor debe estar
iniciado sesión en **este equipo** para que lnwdeck pueda reutilizar la sesión
que ya autorizó:

| Proveedor | Qué hacer en este equipo |
|---|---|
| Gemini / Google / Antigravity | Inicie sesión una vez en **Antigravity IDE** o en la **Gemini CLI**. lnwdeck lee el token automáticamente desde el Administrador de credenciales de Windows. |
| Claude | Ejecute `claude` e inicie sesión una vez. |
| OpenAI Codex | Ejecute `codex login` una vez. |
| Cursor | Inicie sesión una vez en Cursor. |
| Kimi Code | Inicie sesión una vez en Kimi. |
| OpenCode Go | Establezca el ID del espacio de trabajo y la cookie de autenticación en **Configuración → Credenciales del proveedor → OpenCode Go**. |

### Cuota detallada de Gemini / Google / Antigravity

La pantalla **Configuración → Modelos** de Antigravity IDE muestra dos grupos
de cuota: *Gemini Models* y *Claude and GPT models*, cada uno con un límite
*semanal* y otro de *cinco horas*. lnwdeck muestra los mismos números, pero
solo puede leerlos mientras **Antigravity IDE se está ejecutando en este
equipo**, porque Google solo emite esos datos al servidor de lenguaje del IDE.

- **Antigravity IDE abierto** → lnwdeck muestra los porcentajes semanales / de
  5 horas reales, idénticos a los del IDE.
- **Antigravity IDE cerrado** → lnwdeck recurre al endpoint básico de cuota de
  solicitudes, que puede mostrar 100 % aunque ya haya consumido cuota. Abra el
  IDE y pulse **Actualizar todo** para volver a ver los números detallados.

El token de inicio de sesión está en el Administrador de credenciales de
Windows (`gemini:antigravity`). lnwdeck nunca le pide que lo pegue y nunca sale
de su equipo.

### Solución de problemas

- **Sin conexión** — El proveedor no está instalado o no ha iniciado sesión en
  este equipo. Instale/inicie sesión y actualice.
- **Autenticación caducada** — Vuelva a iniciar sesión en el proveedor
  (Antigravity IDE, `claude`, `codex login`, Kimi, ...) y actualice.
- **Gemini muestra 100 %** — Antigravity IDE no está en ejecución, por lo que
  lnwdeck no puede leer la cuota semanal / de 5 horas detallada. Abra el IDE y
  actualice.

---

## Русский

lnwdeck — локальный трекер для Windows, который показывает, сколько токенов
использовал каждый ИИ-инструмент и сколько квоты осталось у вашего провайдера.
Все данные остаются на вашем компьютере: нет аккаунта, сервера или облачной
синхронизации.

### Как собирается квота

lnwdeck читает квоту, опубликованную провайдером. Каждый провайдер должен быть
авторизован **на этом компьютере**, чтобы lnwdeck мог использовать уже
предоставленную вами сессию:

| Провайдер | Что сделать на этом компьютере |
|---|---|
| Gemini / Google / Antigravity | Войдите один раз в **Antigravity IDE** или **Gemini CLI**. lnwdeck автоматически читает токен из диспетчера учётных данных Windows. |
| Claude | Запустите `claude` и войдите один раз. |
| OpenAI Codex | Выполните `codex login` один раз. |
| Cursor | Войдите один раз в Cursor. |
| Kimi Code | Войдите один раз в Kimi. |
| OpenCode Go | Укажите ID рабочего пространства и cookie авторизации в **Настройки → Учётные данные провайдера → OpenCode Go**. |

### Детальная квота Gemini / Google / Antigravity

Экран **Настройки → Модели** в Antigravity IDE показывает две группы квоты:
*Gemini Models* и *Claude and GPT models*, каждая с *недельным* и
*пятичасовым* лимитом. lnwdeck показывает те же числа, но может читать их
только пока **Antigravity IDE запущен на этом компьютере**, поскольку Google
выдаёт эти данные только собственному языковому серверу IDE.

- **Antigravity IDE открыт** → lnwdeck показывает реальные недельные /
  5-часовые проценты, идентичные IDE.
- **Antigravity IDE закрыт** → lnwdeck переключается на базовый endpoint
  квоты запросов, который может показывать 100 %, даже если квота частично
  израсходована. Откройте IDE и нажмите **Обновить все** — детальные числа
  появятся снова.

Токен входа хранится в диспетчере учётных данных Windows
(`gemini:antigravity`). lnwdeck никогда не просит вас вставлять его, и он
никогда не покидает ваш компьютер.

### Устранение неполадок

- **Нет подключения** — Провайдер не установлен или не авторизован на этом
  компьютере. Установите/войдите, затем обновите.
- **Срок действия авторизации истёк** — Повторно войдите у провайдера
  (Antigravity IDE, `claude`, `codex login`, Kimi и т. д.) и обновите.
- **Gemini показывает 100 %** — Antigravity IDE не запущен, поэтому lnwdeck
  не может прочитать детальную недельную / 5-часовую квоту. Откройте IDE и
  обновите.
