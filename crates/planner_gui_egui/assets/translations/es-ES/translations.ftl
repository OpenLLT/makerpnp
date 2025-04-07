theme-button-light = ☀ Claro
theme-button-dark = 🌙 Oscuro
theme-button-system = 💻 Sistema

# format "<language in native language> (<country in native language>)
language-es-ES = Español (España)
language-en-US = English (United States)

menu-top-level-file = Archivo
menu-item-quit = Salir

modal-errors-title = Errores - { $file }
modal-add-pcb-title = Añadir PCB - { $file }
modal-add-phase-title = Añadir fase - { $file }
modal-create-unit-assignment-title = Crear asignación de unidad - { $file }
modal-phase-placement-orderings-title = Ordenación de la colocación de fases - { $phase }

toolbar-button-home = Inicio
toolbar-button-new = Nuevo
toolbar-button-open = Abrir
toolbar-button-save = Guardar
toolbar-button-close-all = Cerrar todo

side-bar-header = Encabezado de la barra lateral
side-bar-footer = Pie de página de la barra lateral
side-bar-item-path = Ruta

project-toolbar-button-show-explorer = Mostrar explorador
project-toolbar-button-generate-artifacts = Generar artefactos
project-toolbar-button-refresh-from-variants = Actualizar desde variantes
project-toolbar-button-remove-unknown-placements = Eliminar ubicaciones desconocidas
project-toolbar-button-add-pcb = Añadir placa
project-toolbar-button-add-phase = Añadir fase
project-toolbar-button-create-unit-assignment = Crear asignacion de unidad

tab-label-home = Inicio
tab-label-new-project = Nuevo proyecto

home-banner = Pestaña de inicio
home-checkbox-label-show-on-startup = Mostrar al inicio

new-project-banner = Nuevo proyecto
form-new-project-input-name = Nombre del proyecto
form-new-project-input-directory = Directorio

project-detail-path = Ruta: { $path }
project-detail-name = Nombre: { $name }

project-overview-tab-label = Visión general
project-overview-header = Visión general
project-overview-detail-name = Nombre: { $name }

project-placements-tab-label = Ubicaciones
project-placements-header = Ubicaciones de proyecto

project-parts-tab-label = Piezas
project-parts-header = Piezas de proyecto

phase-toolbar-add-parts-to-loadout = Añadir piezas a la carga.
phase-toolbar-placement-orderings = Ordenaciones de colocación.

phase-placements-header = Ubicaciones de fase

phase-properties-header = Propiedades de fase
phase-properties-footer = { $count } items

phase-reference = Referencia
phase-load-out-source = Fuente de carga
phase-pcb-side = Lado PCB
phase-process = Proceso

project-load-out-header = Carga

table-load-out-column-reference = Reference
table-load-out-column-manufacturer = Fabricante
table-load-out-column-mpn = MPN

pcb-side-top = Parte superior
pcb-side-bottom = Parte inferior

project-explorer-tab-label = Explorador de proyecto

project-explorer-node-root = Raíz
project-explorer-node-parts = Piezas
project-explorer-node-placements = Ubicaciones
project-explorer-node-phases = Fases
project-explorer-node-phase = { $reference } ({ $process})
project-explorer-node-phase-loadout = { $source }
project-explorer-node-unit-assignments = Asignaciones de unidad
project-explorer-node-unit-assignment = { $name } ({$design_name} - {$variant_name})
project-explorer-node-pcbs = PCBs
project-explorer-node-pcb = { $name } ({ $kind })
project-explorer-node-processes = Procesos
project-explorer-node-process = { $name }

form-button-ok = Aceptar
form-button-cancel = Cancelar

form-add-pcb-input-name = Nombre
form-add-pcb-input-name-placeholder = Nombre de PCB (por ejemplo, 'predeterminado')

form-create-unit-assignment-input-design-name = Nombre del diseño
form-create-unit-assignment-input-design-name-placeholder = Nombre del diseño (por ejemplo, 'mi diseño')
form-create-unit-assignment-input-variant-name = Nombre de la variante
form-create-unit-assignment-input-variant-name-placeholder = Nombre de la variante (por ejemplo, 'default')
form-create-unit-assignment-input-pcb-instance = Instancia PCB
form-create-unit-assignment-input-pcb-instance-placeholder = Un número > 0
form-create-unit-assignment-input-pcb-unit = Unidad PCB
form-create-unit-assignment-input-pcb-unit-placeholder = Un número > 0
form-create-unit-assignment-input-placements-filename = Nombre de archivo de las ubicaciones
form-create-unit-assignment-input-placements-directory = Directorio de ubicaciones

form-common-combo-select = Seleccionar...
form-common-combo-none = Ninguno

form-common-choice-pcb-kind = Tipo
form-common-choice-pcb-kind-single = Individual
form-common-choice-pcb-kind-panel = Panel

form-common-choice-pcb-side = Lado PCB
form-common-choice-pcb-side-top = Alto
form-common-choice-pcb-side-bottom = Bajo

form-common-input-load-out-source = Fuente de carga
form-common-input-phase-reference = Referencia de fase
form-common-choice-process = Proceso

form-option-error-required = * Obligatorio

form-input-error-empty = No puede estar vacío
form-input-error-length = Longitud mínima { $min }
form-choice-empty = Elija una opción

form-input-number-require-greater-than-zero = Requiere un número mayor que cero
form-input-number-require-positive-number = Requiere un número
form-file-not-found = Archivo no encontrado

placement-placed = Colocado
placement-pending = Pendiente

placement-place = Lugar
placement-no-place = No-lugar

placement-status-known = Conocido
placement-status-unknown = Desconocido

table-placements-column-index = #
table-placements-column-object-path = Ruta de objeto
table-placements-column-refdes = Des. de Ref.
table-placements-column-place = Coloca
table-placements-column-placed = ¿Colocado?
table-placements-column-manufacturer = Fabricante
table-placements-column-mpn = MPN
table-placements-column-rotation = Rotación
table-placements-column-x = X
table-placements-column-y = Y
table-placements-column-pcb-side = Lado
table-placements-column-phase = Fase
table-placements-column-status = Estado
table-placements-column-ordering = Ordenación

table-parts-column-index = #
table-parts-column-manufacturer = Fabricante
table-parts-column-mpn = MPN
table-parts-column-processes = Procesos
table-parts-column-ref-des-set = Des. de Ref. Conjunto
table-parts-column-quantity = Cantidad

filter-expression = Buscar...

expanding-header-details = Detalles

#
# egui-data-tables
#

# cell context menu
context-menu-selection-copy = Selección: Copiar
context-menu-selection-cut = Selección: Cortar
context-menu-selection-clear = Selección: Limpiar
context-menu-selection-fill = Selección: Rellenar
context-menu-clipboard-paste = Portapapeles: Pegar
context-menu-clipboard-insert = Portapapeles: Insertar
context-menu-row-duplicate = Fila: Duplicar
context-menu-row-delete = Fila: Eliminar
context-menu-undo = Deshacer
context-menu-redo = Rehacer
# column header context menu
context-menu-hide = Ocultar columna
context-menu-hidden = Columnas ocultas
context-menu-clear-sort = Borrar ordenación
