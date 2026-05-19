import React, { useEffect, useMemo, useState } from "react";
import { FolderIcon, LoadingIcon } from "./Icons";

export type FormDialogField = {
  id: string;
  label: string;
  defaultValue?: string;
  placeholder?: string;
  required?: boolean;
  actionTitle?: string;
  actionIcon?: "folder";
};

interface Props {
  title: string;
  description?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  fields: FormDialogField[];
  visible: boolean;
  onCancel: () => void;
  onConfirm: (values: Record<string, string>) => Promise<void> | void;
  onFieldAction?: (fieldId: string, currentValue: string) => Promise<string | void> | string | void;
}

export default function FormDialog({
  title,
  description,
  confirmLabel = "OK",
  cancelLabel = "Cancel",
  fields,
  visible,
  onCancel,
  onConfirm,
  onFieldAction,
}: Props) {
  const initialValues = useMemo(
    () =>
      Object.fromEntries(fields.map((field) => [field.id, field.defaultValue ?? ""])),
    [fields]
  );
  const [values, setValues] = useState<Record<string, string>>(initialValues);
  const [submitting, setSubmitting] = useState(false);
  const [actionFieldId, setActionFieldId] = useState<string | null>(null);

  useEffect(() => {
    if (visible) {
      setValues(initialValues);
      setSubmitting(false);
      setActionFieldId(null);
    }
  }, [initialValues, visible]);

  if (!visible) return null;

  const isValid = fields.every((field) => !field.required || values[field.id]?.trim());

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!isValid || submitting) return;
    setSubmitting(true);
    try {
      await onConfirm(values);
    } finally {
      setSubmitting(false);
    }
  };

  const handleFieldAction = async (field: FormDialogField) => {
    if (!onFieldAction || !field.actionIcon || actionFieldId || submitting) return;
    setActionFieldId(field.id);
    try {
      const result = await onFieldAction(field.id, values[field.id] ?? "");
      if (typeof result === "string") {
        setValues((current) => ({
          ...current,
          [field.id]: result,
        }));
      }
    } finally {
      setActionFieldId(null);
    }
  };

  return (
    <div className="dialog-backdrop" onClick={onCancel}>
      <form className="dialog-card" onClick={(event) => event.stopPropagation()} onSubmit={handleSubmit}>
        <div className="dialog-header">
          <div className="dialog-title">{title}</div>
          {description ? <div className="dialog-description">{description}</div> : null}
        </div>
        <div className="dialog-fields">
          {fields.map((field) => (
            <label key={field.id} className="dialog-field">
              <span>{field.label}</span>
              <div className="dialog-input-row">
                <input
                  autoFocus={field.id === fields[0]?.id}
                  value={values[field.id] ?? ""}
                  placeholder={field.placeholder}
                  onChange={(event) =>
                    setValues((current) => ({
                      ...current,
                      [field.id]: event.target.value,
                    }))
                  }
                />
                {field.actionIcon && (
                  <button
                    type="button"
                    className="icon-button icon-button-square dialog-input-action"
                    title={field.actionTitle}
                    onClick={() => handleFieldAction(field)}
                    disabled={submitting || actionFieldId === field.id}
                  >
                    {actionFieldId === field.id ? (
                      <LoadingIcon className="button-icon-spin" />
                    ) : field.actionIcon === "folder" ? (
                      <FolderIcon />
                    ) : null}
                  </button>
                )}
              </div>
            </label>
          ))}
        </div>
        <div className="dialog-actions">
          <button type="button" className="toolbar-button" onClick={onCancel} disabled={submitting}>
            {cancelLabel}
          </button>
          <button type="submit" className="btn-merge-toggle" disabled={!isValid || submitting}>
            {confirmLabel}
          </button>
        </div>
      </form>
    </div>
  );
}
