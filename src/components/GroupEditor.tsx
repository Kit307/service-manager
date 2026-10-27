import { useState } from "react";
import type { ServiceRef, WorkspaceGroup } from "../lib/types";
import { refKey, serviceRef, type ServiceEntry } from "../lib/workspace";
import { Icon } from "./Icon";
export function GroupEditor({
  group,
  groups,
  services,
  busy,
  onSave,
  onRemove,
  onCancel,
}: {
  group?: WorkspaceGroup;
  groups: WorkspaceGroup[];
  services: ServiceEntry[];
  busy: boolean;
  onSave: (name: string, services: ServiceRef[]) => void;
  onRemove: () => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState(group?.name ?? "");
  const [members, setMembers] = useState(group?.services ?? []);
  return (
    <div className="editor-scroll">
      <form
        className="service-form group-editor"
        onSubmit={(e) => {
          e.preventDefault();
          onSave(name, members);
        }}
      >
        <h1>{group ? "Group settings" : "New group"}</h1>
        <p>
          A group organizes services in this app. It does not create a folder on
          disk or change AI permissions.
        </p>
        <section className="form-section" aria-label="Group configuration">
        <label>
          Group name
          <input
            required
            maxLength={80}
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
        </label>
        <fieldset className="project-grants">
          <legend>Services in this group</legend>
          <p>Selecting a service moves it from its current group.</p>
          {services.map((entry) => {
            const ref = serviceRef(entry);
            const key = refKey(ref);
            const other = groups.find(
              (g) =>
                g.id !== group?.id && g.services.some((s) => refKey(s) === key),
            );
            return (
              <label key={key}>
                <input
                  type="checkbox"
                  checked={members.some((s) => refKey(s) === key)}
                  onChange={(e) =>
                    setMembers((old) =>
                      e.target.checked
                        ? [...old, ref]
                        : old.filter((s) => refKey(s) !== key),
                    )
                  }
                />
                <span>
                  {entry.service.name}
                  <small>
                    {entry.project.path}/{entry.service.cwd}
                    {other ? ` · ${other.name}` : ""}
                  </small>
                </span>
              </label>
            );
          })}
          {!services.length && (
            <p>Add services first, or save an empty group.</p>
          )}
        </fieldset>
        </section>
        <div className="button-row form-actions" role="group" aria-label="Form actions">
          <button className="primary" disabled={busy || !name.trim()}>
            <Icon name="check" />
            {group ? "Save group" : "Create group"}
          </button>
          <button
            type="button"
            className="secondary"
            disabled={busy}
            onClick={onCancel}
          >
            Cancel
          </button>
          {group && (
            <button
              type="button"
              className="tiny danger-text"
              disabled={busy}
              onClick={onRemove}
            >
              Remove group
            </button>
          )}
        </div>
      </form>
    </div>
  );
}
