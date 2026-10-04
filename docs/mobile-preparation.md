# Preparação futura para mobile e transmissão

Investigação: 4 de outubro de 2026. Este documento descreve possibilidades futuras; não inicia implementação mobile nem altera a prioridade de concluir o desktop Windows. A revisão foi de código, documentação local e fontes oficiais, sem compilar alvos mobile ou testar aparelhos.

## Direção recomendada

Manter o PC como biblioteca e servidor, e criar futuramente um cliente mobile com a interface Svelte, acesso ao catálogo pela rede e adaptadores próprios de reprodução e transmissão. Tauri 2 permite reaproveitar a interface e partes do Rust, mas recursos nativos exigem integrações específicas. Android pode ser a primeira prova; iOS exige macOS/Xcode para desenvolvimento.

Para transmissão DLNA, recomenda-se concentrar descoberta, controle e sessões no PC. O celular escolhe mídia/destino e envia comandos à API Luma; a TV busca o arquivo ou stream convertido diretamente no PC. O vídeo não passa pelo telefone. Essa centralização também permite acompanhar progresso e manter a sessão quando a interface mobile estiver suspensa.

```text
Celular ── catálogo, destinos e comandos ──► PC
PC      ── URL e comandos UPnP ───────────► TV
TV      ── HTTP GET/Range ────────────────► PC
PC      ── mídia original ou conversão ───► TV
```

O modelo UPnP AV distingue Control Point, MediaServer e MediaRenderer; a transferência efetiva ocorre diretamente entre servidor e renderer. Controle UPnP no próprio telefone também é possível, mas acrescenta multicast, callbacks, suspensão mobile e reconciliação de sessão. Não há garantia de interoperabilidade sem testar os aparelhos reais.

## O que já existe

- UI Svelte 5 com composições para telas estreitas, dentro de Tauri 2.
- `src-tauri/media-core` separa SQLite, indexação e identificação de arquivos. É candidato ao compartilhamento; portabilidade mobile ainda não foi compilada/verificada.
- Rust fornece metadados, TMDb, catálogo e histórico locais. `src/lib/platform/desktop.ts` concentra acesso por IPC Tauri, caminhos locais e reprodução.
- Player MPV/VLC integrado ao Windows. `native_player_stub.rs` rejeita reprodução nas outras plataformas.
- Servidor Node de desenvolvimento que lê a biblioteca SQLite, anuncia MediaServer por SSDP e responde a buscas. Expõe ContentDirectory e ConnectionManager.
- Entrega HTTP original com GET/HEAD, Range, cancelamento e validação de caminhos pela biblioteca.
- Alternativa FFmpeg MPEG-TS: remux quando possível, conversão de áudio ou conversão completa para H.264/AAC. Limite de duas conversões simultâneas.
- `/api/library` somente leitura, com lista de mídias e URLs original/compatível. Servidor limitado à rede local e a uma interface/sub-rede.

O servidor é um protótipo de desenvolvimento; ainda não representa serviço de produção empacotado nem compatibilidade comprovada com TVs.

## Lacunas verificadas

### Fronteira entre desktop e mobile

`isDesktopRuntime()` verifica apenas a presença de `__TAURI_INTERNALS__`, portanto também classificaria Tauri mobile como desktop. Será necessário identificar capacidades reais, como catálogo local/remoto, player disponível, transmissão e acesso a arquivos.

O player Windows, HWND, Acrylic e DLLs não são integrações mobile. `torrent_engine.rs` importa `libloading` incondicionalmente, enquanto Cargo o declara apenas para Windows; busca exclusivamente a DLL da ponte libtorrent. Isso é um bloqueio aparente de compilação em outros alvos, além dos recursos Windows no empacotamento. Não foi executada compilação para confirmar todos os bloqueios.

### API e catálogo remoto

A API pública atual retorna uma lista plana com identificação, título, tipo, ano, poster, tamanho, MIME e URLs. Faltam contrato versionado, paginação, detalhes, temporadas/episódios estruturados, capacidades, recursos de legenda, sincronização de progresso, pairing/autenticação e sessões. Não há TLS, contas ou acesso externo.

O cliente não deve depender de caminhos Windows nem copiar o banco do PC como mecanismo de sincronização. Identidade da mídia deve ser compartilhada entre catálogo, recursos de reprodução e histórico.

### Controle de TVs

`media-server/src/ssdp.mjs` anuncia o servidor e responde a `M-SEARCH`; não procura MediaRenderer nem mantém inventário de destinos. `protocol.mjs` e `server.mjs` não implementam cliente SOAP AVTransport/RenderingControl, leitura de capacidades do renderer, eventos da TV ou sessões de transmissão.

As assinaturas já existentes são eventos dos serviços do próprio MediaServer; não equivalem a assinaturas AVTransport de TVs.

### Reprodução e conversão

MIME e publicidade genérica de recursos não comprovam suporte a codec, resolução, áudio, HDR ou legendas. O renderer precisa alcançar a URL LAN do PC: firewall, interface escolhida, Wi-Fi isolado e redes diferentes podem impedir isso. `localhost` e caminhos de arquivo locais não são URLs utilizáveis pela TV.

A conversão atual aceita `?start=seconds`, rejeita Range e não implementa seek temporal DLNA convencional. Reabrir uma conversão em outro offset seria alternativa limitada por aparelho, não seek transparente. Legendas convertidas, seleção de faixas e HDR tone mapping não existem; conversão HDR incompatível é rejeitada. HLS pode complementar a entrega para mobile/Cast, mas não substitui universalmente o formato DLNA atual.

## Fluxo futuro de transmissão DLNA

1. O PC busca MediaRenderer por SSDP, lê descrição em `LOCATION`, resolve URLs dos serviços e examina SCPD/ações. Mantém identidade por UDN, validade dos anúncios, expiração e `byebye`.
2. Consulta `ConnectionManager.GetProtocolInfo` do renderer e compara `Sink` com os recursos da mídia. Cruza isso com probing e perfis/testes do aparelho; declarações genéricas não substituem compatibilidade real.
3. Planejador escolhe original, remux, conversão de áudio ou conversão completa. Fornece URL acessível ao renderer e metadados DIDL-Lite.
4. Envia `AVTransport.SetAVTransportURI` e depois `Play` com velocidade 1. Usa o InstanceID obtido quando disponível ou 0 no caso previsto para HTTP sem `PrepareForConnection`.
5. Encaminha Play/Pause/Stop/Seek conforme ações e modos suportados. Volume usa RenderingControl. A UI só mostra capacidades verificadas para a sessão.
6. Assina `LastChange` e consulta `GetPositionInfo`: posição temporal não é continuamente enviada por LastChange. Progresso vem do estado confirmado do renderer, não apenas do comando enviado.

Sessões futuras precisam registrar mídia, destino, modo de entrega, estado, posição, capacidades e conversão associada; permitir reconexão da UI; tratar erros e mudança de mídia por outro controlador; liberar conversão ao parar; e distinguir fechamento do telefone de Stop na TV. O comportamento com múltiplos controladores precisa de decisão explícita.

## DLNA, Cast e AirPlay

| Destino | Integração futura |
| --- | --- |
| MediaRenderer DLNA | SSDP e SOAP AVTransport; PC entrega mídia por HTTP |
| Google Cast/Chromecast | SDK Sender/Receiver próprio; carregar URL do PC e controlar sessão Cast |
| AirPlay | Integração AVPlayer/AVRoutePickerView e fluxo AirPlay próprio |

Não presumir que toda TV que navega DLNA aceita controle externo como MediaRenderer. Descoberta deve confirmar serviços e ações. Cast e AirPlay não usam os comandos AVTransport desse plano.

Cast admite URLs de mídia; protocolos adaptativos e legendas requerem CORS, ausente no servidor atual. AirPlay deve ser validado separadamente: a API oficial não garante o mesmo caminho direto PC→TV para todas as rotas. Os adaptadores podem compartilhar a experiência de escolher destino, mas devem preservar capacidades e limitações de cada protocolo.

## Reprodução no próprio telefone e permissões

Para assistir no telefone, avaliar plugins com Media3/ExoPlayer no Android e AVPlayer no iOS, mantendo os controles Svelte. Priorizar original compatível e considerar HLS com seek e legendas para conversão. O formato aceito depende do container e dos codecs do aparelho; o player Windows atual não é reaproveitado automaticamente.

Concentrar SSDP no PC evita multicast no telefone, mas não elimina a permissão de acesso à LAN. A documentação Android atual exige `ACCESS_LOCAL_NETWORK` para aplicativos que miram Android 17/SDK 37 ou superior. Para alvos inferiores, seguir as regras específicas da versão; não solicitar essa permissão antes do alvo pertinente. Descoberta direta no telefone também exige tratamento de multicast Wi-Fi/MulticastLock conforme plataforma.

No iOS, considerar descrição de uso da rede local, recusa/revogação, política ATS para HTTP privado e entitlement multicast quando o aplicativo envia/recebe multicast. Endereço manual não elimina a permissão LAN. Desenvolvimento iOS requer macOS/Xcode; não instalar ferramentas mobile durante a fase desktop por causa desta investigação.

Arquivos offline no telefone exigem armazenamento, importação, permissões e lifecycle próprios; não equivalem ao scanner de pastas Windows. Acesso fora de casa também é escopo separado: o servidor atual restringe a mesma sub-rede e não oferece contas/TLS.

## Torrent e indexer opcionais

Uma primeira versão mobile pode controlar futuramente o motor do PC por API autenticada. Portar libtorrent e downloads persistentes para o telefone acrescenta trabalho específico de compilação, armazenamento e execução em segundo plano. A busca local revisada relaciona transferências a TMDb; não foi identificada integração com indexer remoto. API de indexer, credenciais e controle de downloads são capacidades opcionais futuras, separadas da navegação/reprodução básica.

## Sequência futura e decisões pendentes

1. Concluir e validar desktop, catálogo e servidor existentes.
2. Quando houver trabalho futuro em transmissão, validar uma TV real com entrega original e conversão; registrar modelo, firmware, codecs, legendas, pause/seek e rede.
3. Implementar Control Point e sessões no PC, com planejamento de reprodução e estado verdadeiro da TV.
4. Definir API versionada, pairing e controle autenticado; preservar identidade da mídia e histórico.
5. Fazer a primeira prova Android como cliente LAN, com servidor salvo e alternativa manual/QR; validar transmissão e reprodução local separadamente.
6. Depois da prova, avaliar iOS e adaptadores Cast/AirPlay conforme os aparelhos desejados.

Durante a evolução normal do desktop, preservar fronteiras entre domínio/catálogo, transporte e plataforma evita retrabalho; isso não pede migração ou refatoração imediata.

Decisões pendentes: modelos reais de TV; Android/iOS desejados; cliente dependente do PC ou também offline; sincronização por dispositivo/perfil; controle simultâneo e retomada de sessão; necessidade de Cast/AirPlay; uso de torrent/indexer pelo telefone; e eventual acesso externo. A compatibilidade deve ser declarada somente após teste dos destinos concretos.

## Fontes oficiais

- [Tauri — requisitos, incluindo Android/iOS e macOS para iOS](https://v2.tauri.app/start/prerequisites/)
- [Tauri — desenvolvimento de plugins mobile](https://v2.tauri.app/develop/plugins/develop-mobile/)
- [UPnP — MediaRenderer e arquitetura de controle/transferência](https://upnp.org/specs/av/UPnP-av-MediaRenderer-v1-Device.pdf)
- [UPnP — ConnectionManager](https://upnp.org/specs/av/UPnP-av-ConnectionManager-v1-Service.pdf)
- [UPnP — AVTransport, ações, InstanceID e eventos](https://upnp.org/specs/av/UPnP-av-AVTransport-v1-Service.pdf)
- [Google — visão geral Cast](https://developers.google.com/cast/docs/overview)
- [Google — formatos de mídia Cast e CORS](https://developers.google.com/cast/docs/media)
- [Apple — suporte AirPlay](https://developer.apple.com/documentation/avfoundation/supporting-airplay-in-your-app)
- [Android — formatos Media3/ExoPlayer](https://developer.android.com/media/media3/exoplayer/supported-formats)
- [Apple — HTTP Live Streaming](https://developer.apple.com/documentation/HTTP-Live-Streaming)
- [Android — permissão de rede local](https://developer.android.com/privacy-and-security/local-network-permission)
- [Android — WifiManager.MulticastLock](https://developer.android.com/reference/android/net/wifi/WifiManager.MulticastLock)
- [Apple — TN3179, privacidade da rede local](https://developer.apple.com/documentation/technotes/tn3179-understanding-local-network-privacy)
- [Apple — NSAllowsLocalNetworking/ATS](https://developer.apple.com/documentation/bundleresources/information-property-list/nsapptransportsecurity/nsallowslocalnetworking)

Evidências locais: `README.md`, `PROJECT_STATE.md`, `DECISIONS.md`, `media-server/README.md`, `src/lib/platform/desktop.ts`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/src/lib.rs`, `src-tauri/src/native_player_stub.rs`, `src-tauri/src/torrent_engine.rs`, `src-tauri/media-core/Cargo.toml` e `media-server/src/{catalog,protocol,server,ssdp}.mjs`.
