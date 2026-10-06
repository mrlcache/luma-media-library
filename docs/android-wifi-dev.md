# Luma no celular por Wi-Fi (desenvolvimento)

Este fluxo instala **Luma Wi-Fi DEV**, um app Android separado (`app.luma.mobile.wifidev`) que carrega a interface Svelte diretamente do PC. O APK normal continua usando a interface empacotada. A instalação DEV tem dados e pareamento próprios; pareie novamente se precisar dos serviços do PC.

O servidor da interface usa **1424**, sem alterar 1420, 1422, nem a porta do serviço móvel. PC e celular precisam estar na mesma rede, com acesso entre os dispositivos. O servidor precisa permanecer aberto no PC.

## Primeira instalação

Espere a compilação/instalação em andamento no outro chat terminar antes de usar `-Install`. O script copia o código nativo naquele momento para uma pasta exclusiva em `.artifacts/mobile-lan/native-<data>/src-tauri`, sem alterar fontes nativas, projeto Android ou target do app normal. Cada instalação cria uma cópia nova; a primeira compilação pode demorar.

Na raiz do projeto, abra um terminal e inicie o servidor. Substitua o IP pelo IPv4 atual do PC na rede Wi-Fi (verificado em 04/10/2026: `192.168.1.16`):

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\start-android-lan-dev.ps1 -PcAddress 192.168.1.16
```

No celular, abra `http://192.168.1.16:1424` no navegador para confirmar o acesso pela rede. Isso verifica a rede; funções nativas precisam do app DEV. Se o acesso falhar, confira o IP, a rede, o isolamento de clientes do roteador e o Firewall do Windows. Se necessário, libere somente TCP 1424 para a rede privada/local. O script não muda o firewall automaticamente.

Conecte o Galaxy A02s por USB, habilite a depuração USB e autorize o PC. Em **outro terminal**, execute:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\start-android-lan-dev.ps1 -Install -PcAddress 192.168.1.16
```

O script confere ARM32 (`armeabi-v7a`), preserva minSDK 26 e executa `tauri android dev --host <IP> --no-watch` na cópia exclusiva. Não compila desktop ou release. Não usa `adb reverse`, nem habilita ADB por Wi-Fi. Com mais de um aparelho conectado, informe também `-DeviceSerial <serial>`.

Depois da instalação, abra **Luma Wi-Fi DEV**, confirme que carrega pela rede e desconecte o USB. Se o processo de instalação permanecer aberto, encerre apenas esse terminal com Ctrl+C. Mantenha aberto o terminal do servidor.

## Uso diário

Com o app DEV já instalado, basta iniciar o servidor:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\start-android-lan-dev.ps1
```

O script reutiliza o IP salvo em `.artifacts/mobile-lan/session.json`. Abra o app no celular e edite normalmente os componentes Svelte, estilos e código de interface no PC. Vite fornece o canal de HMR; algumas alterações podem causar recarga completa e perder estado temporário. Não rode `-Install` a cada alteração visual.

Depois de reiniciar o servidor, se a tela antiga ficar sem conexão, feche e reabra o app DEV. Se o IP do PC mudar, o app instalado continuará apontando para o antigo: reinstale com o novo `-PcAddress`. Uma reserva DHCP no roteador evita mudanças de IP.

**Rust, Kotlin, dependências nativas, comandos Tauri e permissões Android exigem recompilar e reinstalar com `-Install`.** Alterações no servidor do PC seguem o fluxo próprio daquele servidor. Mudanças apenas na UI não atualizam o código nativo já instalado.

## Isolamento e validação

- Arquivos exclusivos: `scripts/start-android-lan-dev.ps1`, `scripts/mobile-lan.mjs`, `scripts/vite.mobile-lan.config.ts` e `mobile/src-tauri/tauri.mobile-lan.json`.
- Cache Vite, saída SvelteKit, cópias Android e targets Rust ficam em `.artifacts/mobile-lan/`. A configuração LAN é aplicada à cópia, nunca à configuração normal.
- O frontend usa o modo mobile real (`VITE_LUMA_MOBILE=true`, `VITE_LUMA_MOBILE_DEMO=false`). O navegador sozinho não substitui as APIs nativas do app.
- Para preparar uma cópia sem compilar/instalar: `powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\start-android-lan-dev.ps1 -PrepareOnly -PcAddress 192.168.1.16`.
- Validado no PC em 04/10/2026: preparação da cópia isolada; sintaxe PowerShell/Node; página e cliente Vite respondendo HTTP 200 no IP LAN; conexão WebSocket HMR recebendo `connected`; componente Svelte transformado com `import.meta.hot`. O servidor usado nos testes foi encerrado.
- **Ainda não validado no Galaxy:** compilação/instalação deste app DEV, acesso do aparelho pela rede e atualização visual após salvar um componente. Não se afirma que o hot reload no aparelho já foi comprovado. Faça essa confirmação após a instalação inicial, durante a próxima edição de UI autorizada.

Referência: [desenvolvimento móvel no Tauri](https://v2.tauri.app/develop/), que descreve o uso de `devUrl`, endereço de rede e HMR. As opções disponíveis também foram conferidas na CLI instalada no projeto.
