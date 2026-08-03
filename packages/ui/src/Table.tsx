import React from "react";

export interface TableProps {
  caption: string;
  headers: React.ReactNode[];
  children: React.ReactNode;
}

/**
 * A plain data table with a required caption, so assistive technology always
 * knows what the rows describe.
 */
export function Table({ caption, headers, children }: TableProps) {
  return (
    <div className="ui-table-wrap">
      <table className="ui-table">
        <caption className="ui-inline-note">{caption}</caption>
        <thead>
          <tr>
            {headers.map((header, index) => (
              <th key={index} scope="col">
                {header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>{children}</tbody>
      </table>
    </div>
  );
}
